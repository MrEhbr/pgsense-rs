//! Bootstrap SQL applied to the *source* PostgreSQL database (the one being
//! replicated) at pipeline startup when [`StoreType::Memory`] is used.
//!
//! The etl pipeline issues `etl.describe_table_schema(oid)` and
//! `etl.describe_table_identity(oid)` against the source DB during table
//! sync. With [`StoreType::Postgres`] etl's own migrations install these
//! functions; with [`StoreType::Memory`] we install them ourselves here.
//!
//! What we deliberately skip: the `supabase_etl_ddl_message_trigger` event
//! trigger, `etl.emit_schema_change_messages()`, and the
//! `supabase_etl.skip_ddl_log` table. Trade-off: with the trigger absent,
//! `ALTER TABLE` on a published table during streaming makes etl's apply
//! worker fail with `CorruptedTableSchema` on the first DML that uses the new
//! column set. The supervisor then reconnects (full pipeline rebuild,
//! re-fetches schema via `describe_table_schema`) and resumes from the
//! replication slot, so memory-store deployments recover automatically but
//! eat an error log + ~1s gap per `ALTER TABLE`. Use [`StoreType::Postgres`]
//! (which installs the full DDL trigger via etl's migrations) for deployments
//! that experience frequent schema changes.
//!
//! [`StoreType::Memory`]: crate::pipeline::config::StoreType::Memory
//! [`StoreType::Postgres`]: crate::pipeline::config::StoreType::Postgres

use anyhow::{Context, Result};
use etl::config::PgConnectionConfig;
use secrecy::ExposeSecret;
use sqlx::{
    Connection, Executor, PgConnection,
    postgres::{PgConnectOptions, PgSslMode},
};
use tracing::debug;

/// SQL installed on the source database. Idempotent: uses
/// `create schema if not exists` and `create or replace function`.
const SOURCE_BOOTSTRAP_SQL: &str = r#"
create schema if not exists etl;

create or replace function etl.describe_table_schema(
    p_table pg_catalog.oid
) returns table (
    attname pg_catalog.text,
    attnum pg_catalog.int4,
    atttypid pg_catalog.oid,
    typname pg_catalog.text,
    formatted_type pg_catalog.text,
    atttypmod pg_catalog.int4,
    attnotnull pg_catalog.bool,
    atthasdef pg_catalog.bool,
    default_expression pg_catalog.text,
    attidentity pg_catalog.text,
    atthasmissing pg_catalog.bool
)
language sql
stable
strict
set search_path = pg_catalog
as
$fnc$
select
    a.attname::pg_catalog.text,
    a.attnum::pg_catalog.int4,
    a.atttypid,
    t.typname::pg_catalog.text,
    pg_catalog.format_type(a.atttypid, a.atttypmod)::pg_catalog.text,
    a.atttypmod::pg_catalog.int4,
    a.attnotnull,
    a.atthasdef,
    case
        when a.atthasdef then pg_catalog.pg_get_expr(ad.adbin, ad.adrelid)::pg_catalog.text
        else null
    end,
    nullif(a.attidentity, '')::pg_catalog.text,
    a.atthasmissing
from pg_catalog.pg_attribute a
join pg_catalog.pg_type t
  on t.oid = a.atttypid
left join pg_catalog.pg_attrdef ad
  on ad.adrelid = a.attrelid
 and ad.adnum = a.attnum
where a.attrelid = p_table
  and a.attnum > 0
  and not a.attisdropped
  and a.attgenerated = ''
order by a.attnum;
$fnc$;

create or replace function etl.describe_table_identity(
    p_table pg_catalog.oid
) returns pg_catalog.jsonb
language sql
stable
strict
set search_path = pg_catalog
as
$fnc$
with rel as (
    select c.relreplident
    from pg_catalog.pg_class c
    where c.oid = p_table
),
direct_parent as (
    select i.inhparent as parent_oid
    from pg_catalog.pg_inherits i
    where i.inhrelid = p_table
    order by i.inhseqno
    limit 1
),
primary_key_cols as (
    select
        x.attnum::pg_catalog.int4 as attnum,
        x.n::pg_catalog.int4 as position
    from pg_catalog.pg_constraint con
    cross join lateral unnest(con.conkey) with ordinality as x(attnum, n)
    where con.conrelid = p_table
      and con.contype = 'p'
),
parent_primary_key_cols as (
    select
        x.attnum::pg_catalog.int4 as attnum,
        x.n::pg_catalog.int4 as position
    from direct_parent dp
    join pg_catalog.pg_constraint con
      on con.conrelid = dp.parent_oid
     and con.contype = 'p'
    cross join lateral unnest(con.conkey) with ordinality as x(attnum, n)
),
effective_primary_key_cols as (
    select
        pkc.attnum,
        pkc.position
    from primary_key_cols pkc
    union all
    select
        ppkc.attnum,
        ppkc.position
    from parent_primary_key_cols ppkc
    where not exists (
        select 1
        from primary_key_cols pkc
    )
),
replica_identity_index as (
    select ic.relname as index_name
    from pg_catalog.pg_index i
    join pg_catalog.pg_class ic
      on ic.oid = i.indexrelid
    where i.indrelid = p_table
      and i.indisreplident
),
replica_identity_cols as (
    select
        x.attnum::pg_catalog.int4 as attnum,
        x.n::pg_catalog.int4 as position
    from pg_catalog.pg_index i
    cross join lateral unnest(i.indkey) with ordinality as x(attnum, n)
    where i.indrelid = p_table
      and i.indisreplident
      and x.n <= i.indnkeyatts
      and x.attnum > 0
)
select pg_catalog.jsonb_build_object(
    'primary_key_attnums',
    coalesce(
        (
            select pg_catalog.jsonb_agg(epkc.attnum order by epkc.position)
            from effective_primary_key_cols epkc
        ),
        '[]'::pg_catalog.jsonb
    ),
    'relreplident',
    r.relreplident::pg_catalog.text,
    'replica_identity_index_relname',
    (
        select rii.index_name
        from replica_identity_index rii
        limit 1
    ),
    'replica_identity_index_attnums',
    coalesce(
        (
            select pg_catalog.jsonb_agg(ric.attnum order by ric.position)
            from replica_identity_cols ric
        ),
        '[]'::pg_catalog.jsonb
    )
)
from rel r;
$fnc$;
"#;

/// Apply the source-side bootstrap SQL. Idempotent — uses `create or replace`
/// and `create schema if not exists` throughout.
pub async fn apply(config: &PgConnectionConfig) -> Result<()> {
    let ssl_mode = if config.tls.enabled { PgSslMode::VerifyFull } else { PgSslMode::Prefer };
    let mut opts = PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .database(&config.name)
        .username(&config.username)
        .ssl_mode(ssl_mode);
    if let Some(password) = &config.password {
        opts = opts.password(password.expose_secret());
    }

    let mut conn = PgConnection::connect_with(&opts)
        .await
        .context("failed to connect to source database for bootstrap")?;

    // Quiet the routine DDL notices so startup logs stay focused on
    // phase-level events.
    conn.execute("set client_min_messages = warning;")
        .await
        .context("set client_min_messages")?;

    conn.execute(SOURCE_BOOTSTRAP_SQL)
        .await
        .context("apply source bootstrap SQL")?;

    debug!("source database bootstrap applied");
    Ok(())
}
