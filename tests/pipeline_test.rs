use std::time::Duration;

use pgsense_rs::{
    events::Action,
    pipeline::{
        config::{PipelineSettings, PostgresConfig, PostgresStoreConfig, SqliteStoreConfig, StoreType},
        runner::PipelineRunner,
    },
};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ImageExt, runners::AsyncRunner},
};
use tokio::time::timeout;

const CREATE_TABLE: &str = "CREATE TABLE test_data (id SERIAL PRIMARY KEY, col_a TEXT NOT NULL, col_b TEXT)";
const PUBLICATION: &str = "pgsense_pub";

/// Connect to PostgreSQL with retries, spawning the connection task in the
/// background.
async fn pg_client(conn_str: &str) -> tokio_postgres::Client {
    let max_attempts = 10;
    for attempt in 1..=max_attempts {
        match tokio_postgres::connect(conn_str, tokio_postgres::NoTls).await {
            Ok((client, connection)) => {
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("pg connection error: {e}");
                    }
                });
                return client;
            },
            Err(e) => {
                if attempt == max_attempts {
                    panic!("failed to connect after {max_attempts} attempts: {e}");
                }
                eprintln!("connection attempt {attempt}/{max_attempts} failed: {e}, retrying...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            },
        }
    }
    unreachable!()
}

async fn start_pg_container() -> (testcontainers_modules::testcontainers::ContainerAsync<Postgres>, String, u16) {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .with_cmd([
            "postgres",
            "-c",
            "wal_level=logical",
            "-c",
            "max_replication_slots=4",
            "-c",
            "max_wal_senders=4",
        ])
        .start()
        .await
        .expect("failed to start postgres container");

    let host = container.get_host().await.expect("get host").to_string();
    let port = container.get_host_port_ipv4(5432).await.expect("get port");
    (container, host, port)
}

async fn setup_database(host: &str, port: u16) -> tokio_postgres::Client {
    let conn_str = format!("host={host} port={port} user=postgres password=postgres dbname=postgres");
    let client = pg_client(&conn_str).await;
    client
        .execute(CREATE_TABLE, &[])
        .await
        .expect("create table");
    client
        .execute(&format!("CREATE PUBLICATION {PUBLICATION} FOR TABLE test_data"), &[])
        .await
        .expect("create publication");
    client
}

// ---------------------------------------------------------------------------
// Test bodies
// ---------------------------------------------------------------------------

async fn test_insert_events(pg_cfg: &PostgresConfig, client: &tokio_postgres::Client, settings: &PipelineSettings) {
    let mut runner = PipelineRunner::new(1, pg_cfg, settings)
        .await
        .expect("create pipeline runner");

    let mut event_rx = runner.take_event_receiver().expect("take event receiver");

    runner.start().await.expect("start pipeline");
    tokio::time::sleep(Duration::from_secs(2)).await;

    client
        .execute(
            "INSERT INTO test_data (col_a, col_b) VALUES ($1, $2)",
            &[&"value_a1", &"value_b1"],
        )
        .await
        .expect("insert row");

    let events = timeout(Duration::from_secs(10), event_rx.recv())
        .await
        .expect("timed out waiting for scan events")
        .expect("event channel closed unexpectedly");

    assert!(!events.is_empty(), "expected at least one scan event");

    let event = &events[0];
    assert_eq!(event.action, Action::Insert);

    let col_a = event.columns.iter().find(|c| c.name == "col_a");
    assert_eq!(col_a.map(|c| c.value.as_deref()), Some(Some("value_a1")));

    let col_b = event.columns.iter().find(|c| c.name == "col_b");
    assert_eq!(col_b.map(|c| c.value.as_deref()), Some(Some("value_b1")));
}

async fn test_restart_catchup(pg_cfg: &PostgresConfig, client: &tokio_postgres::Client, settings: &PipelineSettings) {
    let pipeline_id = 2;

    // Phase 1: start pipeline to create the replication slot
    let mut runner = PipelineRunner::new(pipeline_id, pg_cfg, settings)
        .await
        .expect("create pipeline runner");

    let mut event_rx = runner.take_event_receiver().expect("take event receiver");

    runner.start().await.expect("start pipeline");
    tokio::time::sleep(Duration::from_secs(2)).await;

    client
        .execute("INSERT INTO test_data (col_a, col_b) VALUES ($1, $2)", &[&"before", &"row1"])
        .await
        .expect("insert before-shutdown row");

    let events = timeout(Duration::from_secs(10), event_rx.recv())
        .await
        .expect("timed out waiting for pre-shutdown event")
        .expect("channel closed");
    assert!(!events.is_empty(), "expected pre-shutdown event");

    // Phase 2: shut down the pipeline
    runner.shutdown().await.expect("shutdown pipeline");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Phase 3: insert data while pipeline is down
    client
        .execute(
            "INSERT INTO test_data (col_a, col_b) VALUES ($1, $2)",
            &[&"while-down", &"row2"],
        )
        .await
        .expect("insert while-down row");

    // Phase 4: restart pipeline with the same pipeline_id and same store
    let mut runner2 = PipelineRunner::new(pipeline_id, pg_cfg, settings)
        .await
        .expect("create pipeline runner (restart)");

    let mut event_rx2 = runner2
        .take_event_receiver()
        .expect("take event receiver (restart)");

    runner2.start().await.expect("start pipeline (restart)");

    let mut found_while_down = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);

    while tokio::time::Instant::now() < deadline {
        match timeout(Duration::from_secs(10), event_rx2.recv()).await {
            Ok(Some(events)) => {
                for event in &events {
                    let col_a = event.columns.iter().find(|c| c.name == "col_a");
                    if col_a.and_then(|c| c.value.as_deref()) == Some("while-down") {
                        assert_eq!(event.action, Action::Insert);
                        let col_b = event.columns.iter().find(|c| c.name == "col_b");
                        assert_eq!(col_b.and_then(|c| c.value.as_deref()), Some("row2"));
                        found_while_down = true;
                    }
                }
                if found_while_down {
                    break;
                }
            },
            Ok(None) => panic!("event channel closed unexpectedly"),
            Err(_) => break,
        }
    }

    assert!(
        found_while_down,
        "pipeline did not catch up with data inserted while it was down"
    );
}

// ---------------------------------------------------------------------------
// Insert events — one test per store variant
// ---------------------------------------------------------------------------

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_receives_insert_events() {
    let (_container, host, port) = start_pg_container().await;
    let client = setup_database(&host, port).await;
    let pg_cfg = PostgresConfig {
        host: host.clone(),
        port,
        dbname: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Some("postgres".to_string()),
        publication: PUBLICATION.to_string(),
        ..Default::default()
    };

    test_insert_events(&pg_cfg, &client, &PipelineSettings::default()).await;
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_receives_insert_events_sqlite() {
    let (_container, host, port) = start_pg_container().await;
    let client = setup_database(&host, port).await;
    let pg_cfg = PostgresConfig {
        host: host.clone(),
        port,
        dbname: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Some("postgres".to_string()),
        publication: PUBLICATION.to_string(),
        ..Default::default()
    };
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let db_path = tmp_dir.path().join("test-state.db");
    let settings = PipelineSettings {
        store: StoreType::Sqlite(SqliteStoreConfig {
            path: db_path.to_str().unwrap().to_string(),
        }),
        ..PipelineSettings::default()
    };

    test_insert_events(&pg_cfg, &client, &settings).await;
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_receives_insert_events_postgres_store() {
    let (_container, host, port) = start_pg_container().await;
    let client = setup_database(&host, port).await;
    let pg_cfg = PostgresConfig {
        host: host.clone(),
        port,
        dbname: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Some("postgres".to_string()),
        publication: PUBLICATION.to_string(),
        ..Default::default()
    };
    let settings = PipelineSettings {
        store: StoreType::Postgres(PostgresStoreConfig {
            host: host.clone(),
            port,
            password: Some("postgres".to_string()),
            ..Default::default()
        }),
        ..PipelineSettings::default()
    };

    test_insert_events(&pg_cfg, &client, &settings).await;
}

// ---------------------------------------------------------------------------
// Restart catch-up — persistent stores only (Memory loses state on restart)
// ---------------------------------------------------------------------------

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_catches_up_after_restart_sqlite() {
    let (_container, host, port) = start_pg_container().await;
    let client = setup_database(&host, port).await;
    let pg_cfg = PostgresConfig {
        host: host.clone(),
        port,
        dbname: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Some("postgres".to_string()),
        publication: PUBLICATION.to_string(),
        ..Default::default()
    };
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let db_path = tmp_dir.path().join("test-state.db");
    let settings = PipelineSettings {
        store: StoreType::Sqlite(SqliteStoreConfig {
            path: db_path.to_str().unwrap().to_string(),
        }),
        ..PipelineSettings::default()
    };

    test_restart_catchup(&pg_cfg, &client, &settings).await;
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_catches_up_after_restart_postgres_store() {
    let (_container, host, port) = start_pg_container().await;
    let client = setup_database(&host, port).await;
    let pg_cfg = PostgresConfig {
        host: host.clone(),
        port,
        dbname: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Some("postgres".to_string()),
        publication: PUBLICATION.to_string(),
        ..Default::default()
    };
    let settings = PipelineSettings {
        store: StoreType::Postgres(PostgresStoreConfig {
            host: host.clone(),
            port,
            password: Some("postgres".to_string()),
            ..Default::default()
        }),
        ..PipelineSettings::default()
    };

    test_restart_catchup(&pg_cfg, &client, &settings).await;
}
