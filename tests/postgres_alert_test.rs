mod support;

use std::time::Duration;

use pgsense_rs::{
    alerts::{config::PostgresAlertConfig, postgres::PostgresChannel},
    pipeline::config::TlsSettings,
    rules::config::Severity,
    scanner::Finding,
};
use secrecy::SecretString;
use sqlx::{PgPool, postgres::PgConnectOptions};
use support::PgContainer;

fn test_finding() -> Finding {
    Finding {
        database: "localhost/testdb".to_string(),
        rule_id: "test-rule".to_string(),
        description: "test description".to_string(),
        category: "test".to_string(),
        severity: Severity::High,
        schema_name: "public".to_string(),
        table_name: "events".to_string(),
        column_name: "data".to_string(),
        masked_sample: "***masked***".to_string(),
        value_hash: 0,
        primary_keys: vec![("id".to_string(), "1".to_string())],
        lsn: 1,
        channels: None,
    }
}

struct TestHarness {
    channel: PostgresChannel,
    verify_pool: PgPool,
    _pg: PgContainer,
}

async fn setup() -> TestHarness {
    let pg = PgContainer::start().await;

    let config = PostgresAlertConfig {
        name: None,
        host: pg.host.clone(),
        port: pg.port,
        dbname: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Some(SecretString::from("postgres")),
        schema: "pgsense_alerts".to_string(),
        table: "findings".to_string(),
        tls: TlsSettings::default(),
    };

    let mut last_err = None;
    let mut channel = None;
    for _ in 0..10 {
        match PostgresChannel::new(&config).await {
            Ok(ch) => {
                channel = Some(ch);
                break;
            },
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            },
        }
    }
    let channel = channel.unwrap_or_else(|| panic!("failed to create channel after retries: {:?}", last_err));

    let verify_pool = PgPool::connect_with(
        PgConnectOptions::new()
            .host(&pg.host)
            .port(pg.port)
            .database("postgres")
            .username("postgres")
            .password("postgres"),
    )
    .await
    .expect("failed to create verification pool");

    TestHarness { channel, verify_pool, _pg: pg }
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn send_inserts_finding() {
    let h = setup().await;

    h.channel.send(&test_finding()).await.unwrap();

    let cnt: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "pgsense_alerts"."findings""#)
        .fetch_one(&h.verify_pool)
        .await
        .unwrap();
    assert_eq!(cnt, 1);
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn send_stores_all_fields() {
    let h = setup().await;

    h.channel.send(&test_finding()).await.unwrap();

    let row: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        serde_json::Value,
    ) = sqlx::query_as(
        r#"SELECT database, rule_id, category, severity, schema_name, table_name, column_name, masked_sample, lsn, primary_key
           FROM "pgsense_alerts"."findings" LIMIT 1"#,
    )
    .fetch_one(&h.verify_pool)
    .await
    .unwrap();

    assert_eq!(row.0, "localhost/testdb");
    assert_eq!(row.1, "test-rule");
    assert_eq!(row.2, "test");
    assert_eq!(row.3, "HIGH");
    assert_eq!(row.4, "public");
    assert_eq!(row.5, "events");
    assert_eq!(row.6, "data");
    assert_eq!(row.7, "***masked***");
    assert_eq!(row.8, 1);
    assert_eq!(row.9, serde_json::json!({"id": "1"}));
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn multiple_sends() {
    let h = setup().await;

    for i in 0..5 {
        let mut finding = test_finding();
        finding.column_name = format!("col_{i}");
        h.channel.send(&finding).await.unwrap();
    }

    let cnt: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "pgsense_alerts"."findings""#)
        .fetch_one(&h.verify_pool)
        .await
        .unwrap();
    assert_eq!(cnt, 5);
}
