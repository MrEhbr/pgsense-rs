use anyhow::{Context, Result};
use secrecy::ExposeSecret;
use sqlx::{
    Executor, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use tracing::info;

use super::config::PostgresAlertConfig;
use crate::scanner::Finding;

#[derive(Debug)]
pub struct PostgresChannel {
    pool: PgPool,
    table_fqn: String,
}

impl PostgresChannel {
    pub async fn new(config: &PostgresAlertConfig) -> Result<Self> {
        let schema = &config.schema;
        let table = &config.table;

        anyhow::ensure!(
            is_valid_identifier(schema),
            "invalid schema name: must contain only ASCII alphanumeric characters and underscores"
        );
        anyhow::ensure!(
            is_valid_identifier(table),
            "invalid table name: must contain only ASCII alphanumeric characters and underscores"
        );

        let ssl_mode = if config.tls.enabled { PgSslMode::VerifyFull } else { PgSslMode::Prefer };
        let mut connect_options = PgConnectOptions::new()
            .host(&config.host)
            .port(config.port)
            .database(&config.dbname)
            .username(&config.username)
            .ssl_mode(ssl_mode);

        if let Some(password) = &config.password {
            connect_options = connect_options.password(password.expose_secret());
        }

        let schema_for_hook = schema.clone();
        let pool = PgPoolOptions::new()
            .min_connections(0)
            .max_connections(2)
            .idle_timeout(Some(std::time::Duration::from_secs(30)))
            .after_connect(move |conn, _meta| {
                let schema = schema_for_hook.clone();
                Box::pin(async move {
                    conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
                        .await?;
                    Ok(())
                })
            })
            .connect_with(connect_options)
            .await
            .context("failed to connect to postgres alert store")?;

        pool.execute(format!(r#"CREATE SCHEMA IF NOT EXISTS "{schema}""#).as_str())
            .await
            .context("failed to create schema for postgres alert store")?;

        let create_table = format!(
            r#"CREATE TABLE IF NOT EXISTS "{schema}"."{table}" (
                id BIGSERIAL PRIMARY KEY,
                database TEXT NOT NULL,
                rule_id TEXT NOT NULL,
                description TEXT NOT NULL,
                category TEXT NOT NULL,
                severity TEXT NOT NULL,
                schema_name TEXT NOT NULL,
                table_name TEXT NOT NULL,
                column_name TEXT NOT NULL,
                masked_sample TEXT NOT NULL,
                primary_key JSONB NOT NULL DEFAULT '{{}}',
                lsn BIGINT NOT NULL,
                detected_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )"#,
        );
        pool.execute(create_table.as_str())
            .await
            .context("failed to create findings table")?;

        let table_fqn = format!(r#""{schema}"."{table}""#);

        info!(schema, table, "postgres alert channel initialized");

        Ok(Self { pool, table_fqn })
    }

    pub async fn send(&self, finding: &Finding) -> Result<()> {
        let primary_key: serde_json::Value = finding
            .primary_keys
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        let query = format!(
            r#"INSERT INTO {} (database, rule_id, description, category, severity, schema_name, table_name, column_name, masked_sample, primary_key, lsn)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
            self.table_fqn,
        );

        sqlx::query(&query)
            .bind(&finding.database)
            .bind(&finding.rule_id)
            .bind(&finding.description)
            .bind(&finding.category)
            .bind(finding.severity.to_string())
            .bind(&finding.schema_name)
            .bind(&finding.table_name)
            .bind(&finding.column_name)
            .bind(&finding.masked_sample)
            .bind(&primary_key)
            .bind(finding.lsn as i64)
            .execute(&self.pool)
            .await
            .context("failed to insert finding into postgres")?;

        Ok(())
    }
}

fn is_valid_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("pgsense")]
    #[case("my_schema_2")]
    #[case("findings")]
    fn valid_identifier(#[case] input: &str) {
        assert!(is_valid_identifier(input));
    }

    #[rstest]
    #[case("", "empty")]
    #[case("my-schema", "hyphen")]
    #[case("schema; DROP TABLE", "semicolon")]
    #[case("a.b", "dot")]
    fn invalid_identifier(#[case] input: &str, #[case] _reason: &str) {
        assert!(!is_valid_identifier(input));
    }

    #[tokio::test]
    async fn invalid_schema_rejected() {
        let config = PostgresAlertConfig {
            schema: "bad-schema".to_string(),
            ..Default::default()
        };
        let result = PostgresChannel::new(&config).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("invalid schema name")
        );
    }

    #[tokio::test]
    async fn invalid_table_rejected() {
        let config = PostgresAlertConfig {
            table: "drop; --".to_string(),
            ..Default::default()
        };
        let result = PostgresChannel::new(&config).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("invalid table name")
        );
    }
}
