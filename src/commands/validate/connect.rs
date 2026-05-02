use std::time::Duration;

use anyhow::{Context, Result};
use secrecy::{ExposeSecret, SecretString};
use sqlx::{
    Executor,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};

use super::ValidationReport;
use crate::{
    alerts::config::{AlertsConfig, ChannelRef},
    pipeline::config::DatabaseConfig,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

async fn connect_pg(host: &str, port: u16, dbname: &str, username: &str, password: Option<&SecretString>, tls_enabled: bool) -> Result<()> {
    let ssl_mode = if tls_enabled { PgSslMode::VerifyFull } else { PgSslMode::Prefer };
    let mut opts = PgConnectOptions::new()
        .host(host)
        .port(port)
        .database(dbname)
        .username(username)
        .ssl_mode(ssl_mode);
    if let Some(password) = password {
        opts = opts.password(password.expose_secret());
    }
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(CONNECT_TIMEOUT)
        .connect_with(opts)
        .await
        .context("connect failed")?;
    pool.execute("SELECT 1").await.context("SELECT 1 failed")?;
    pool.close().await;
    Ok(())
}

pub(super) async fn check_alerts(alerts: &AlertsConfig, report: &mut ValidationReport) {
    let client = reqwest::Client::builder()
        .timeout(CONNECT_TIMEOUT)
        .build()
        .expect("reqwest client builds");

    for (name, ch) in alerts.channels() {
        match ch {
            ChannelRef::Log | ChannelRef::Stdout | ChannelRef::Jsonl(_) => {},
            ChannelRef::Webhook(w) => {
                if !(w.url.starts_with("http://") || w.url.starts_with("https://")) {
                    report.warn(
                        "alerts",
                        format!("webhook '{name}': skipped connectivity check (structural error)"),
                    );
                    continue;
                }
                match w.head_check(&client).await {
                    Ok(status) => report.ok("alerts", format!("webhook '{name}': server responded to HEAD ({status})")),
                    Err(e) => report.error("alerts", format!("webhook '{name}': {e}")),
                }
            },
            ChannelRef::Slack(s) => {
                if s.token.expose_secret().is_empty() {
                    report.warn(
                        "alerts",
                        format!("slack '{name}': skipped connectivity check (structural error)"),
                    );
                    continue;
                }
                match s.auth_test(&client).await {
                    Ok(()) => report.ok("alerts", format!("slack '{name}': auth.test succeeded")),
                    Err(e) => report.error("alerts", format!("slack '{name}': {e}")),
                }
            },
            ChannelRef::Postgres(pg) => {
                match connect_pg(
                    &pg.host,
                    pg.port,
                    &pg.dbname,
                    &pg.username,
                    pg.password.as_ref(),
                    pg.tls.enabled,
                )
                .await
                {
                    Ok(()) => report.ok("alerts", format!("postgres '{name}': SELECT 1 succeeded")),
                    Err(e) => report.error("alerts", format!("postgres '{name}': {e:#}")),
                }
            },
        }
    }
}

pub(super) async fn check_databases(databases: &[DatabaseConfig], report: &mut ValidationReport) {
    for db in databases {
        let id = db.database_id();
        match connect_pg(
            &db.host,
            db.port,
            &db.dbname,
            &db.username,
            db.password.as_ref(),
            db.tls.enabled,
        )
        .await
        {
            Ok(()) => report.ok("database", format!("{id}: connection successful")),
            Err(e) => report.error("database", format!("{id}: {e:#}")),
        }
    }
}
