mod support;

use std::time::Duration;

use pgsense_rs::{
    events::Action,
    pipeline::{
        config::{DatabaseConfig, PipelineSettings, PostgresStoreConfig, SqliteStoreConfig, StoreType},
        runner::PipelineRunner,
    },
};
use secrecy::SecretString;
use support::PgContainer;
use tokio::{sync::mpsc, time::timeout};

async fn insert_marker(client: &tokio_postgres::Client, val: &str) {
    client
        .execute("INSERT INTO test_data (col_a, col_b) VALUES ($1, $2)", &[&val, &"row"])
        .await
        .expect("insert marker row");
}

async fn test_insert_events(db_cfg: &DatabaseConfig, client: &tokio_postgres::Client, settings: &PipelineSettings) {
    let (event_tx, mut event_rx) = mpsc::channel(1024);
    let mut runner = PipelineRunner::new(1, db_cfg, settings, event_tx)
        .await
        .expect("create pipeline runner");

    runner.start().await.expect("start pipeline");
    tokio::time::sleep(Duration::from_millis(500)).await;

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

    runner.shutdown().await.expect("shutdown pipeline");
}

async fn test_restart_catchup(db_cfg: &DatabaseConfig, client: &tokio_postgres::Client, settings: &PipelineSettings) {
    let pipeline_id = 2;

    // Phase 1: start pipeline to create the replication slot
    let (event_tx, mut event_rx) = mpsc::channel(1024);
    let mut runner = PipelineRunner::new(pipeline_id, db_cfg, settings, event_tx)
        .await
        .expect("create pipeline runner");

    runner.start().await.expect("start pipeline");
    tokio::time::sleep(Duration::from_millis(500)).await;

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
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Phase 3: insert data while pipeline is down
    client
        .execute(
            "INSERT INTO test_data (col_a, col_b) VALUES ($1, $2)",
            &[&"while-down", &"row2"],
        )
        .await
        .expect("insert while-down row");

    // Phase 4: restart pipeline with the same pipeline_id and same store
    let (event_tx2, mut event_rx2) = mpsc::channel(1024);
    let mut runner2 = PipelineRunner::new(pipeline_id, db_cfg, settings, event_tx2)
        .await
        .expect("create pipeline runner (restart)");

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

    runner2.signal_shutdown();
    runner2.wait().await.expect("shutdown runner2");
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_receives_insert_events() {
    let pg = PgContainer::start_with_wal().await;
    let client = pg.setup_database().await;
    let db_cfg = pg.db_config("postgres");

    test_insert_events(&db_cfg, &client, &PipelineSettings::default()).await;
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_receives_insert_events_sqlite() {
    let pg = PgContainer::start_with_wal().await;
    let client = pg.setup_database().await;
    let db_cfg = pg.db_config("postgres");
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let db_path = tmp_dir.path().join("test-state.db");
    let settings = PipelineSettings {
        store: StoreType::Sqlite(SqliteStoreConfig {
            path: db_path.to_str().unwrap().to_string(),
        }),
        ..PipelineSettings::default()
    };

    test_insert_events(&db_cfg, &client, &settings).await;
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_receives_insert_events_postgres_store() {
    let pg = PgContainer::start_with_wal().await;
    let client = pg.setup_database().await;
    let db_cfg = pg.db_config("postgres");
    let settings = PipelineSettings {
        store: StoreType::Postgres(PostgresStoreConfig {
            host: pg.host.clone(),
            port: pg.port,
            password: Some(SecretString::from("postgres")),
            ..Default::default()
        }),
        ..PipelineSettings::default()
    };

    test_insert_events(&db_cfg, &client, &settings).await;
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_catches_up_after_restart_sqlite() {
    let pg = PgContainer::start_with_wal().await;
    let client = pg.setup_database().await;
    let db_cfg = pg.db_config("postgres");
    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let db_path = tmp_dir.path().join("test-state.db");
    let settings = PipelineSettings {
        store: StoreType::Sqlite(SqliteStoreConfig {
            path: db_path.to_str().unwrap().to_string(),
        }),
        ..PipelineSettings::default()
    };

    test_restart_catchup(&db_cfg, &client, &settings).await;
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn two_pipelines_merge_events_from_two_databases() {
    let pg = PgContainer::start_with_wal().await;
    let client1 = pg.setup_database().await;
    let client2 = pg.create_database("testdb2").await;

    let db_cfg1 = pg.db_config("postgres");
    let db_cfg2 = pg.db_config("testdb2");
    let settings = PipelineSettings::default();

    // Shared channel — both pipelines write directly to the same sender
    let (event_tx, mut event_rx) = mpsc::channel(2048);

    let mut runner1 = PipelineRunner::new(10, &db_cfg1, &settings, event_tx.clone())
        .await
        .expect("create runner1");
    let mut runner2 = PipelineRunner::new(11, &db_cfg2, &settings, event_tx)
        .await
        .expect("create runner2");

    runner1.start().await.expect("start runner1");
    runner2.start().await.expect("start runner2");
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Insert into both databases. Under heavy parallel load, the replication
    // slot for db2 may not be ready yet — the recv loop below retries the
    // insert on timeout to handle this race.
    insert_marker(&client1, "from-db1").await;
    insert_marker(&client2, "from-db2").await;

    let mut found_db1 = false;
    let mut found_db2 = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);

    while tokio::time::Instant::now() < deadline && (!found_db1 || !found_db2) {
        match timeout(Duration::from_secs(5), event_rx.recv()).await {
            Ok(Some(events)) => {
                for event in &events {
                    let col_a = event.columns.iter().find(|c| c.name == "col_a");
                    match col_a.and_then(|c| c.value.as_deref()) {
                        Some("from-db1") => {
                            assert_eq!(event.database, format!("{}/postgres", pg.host));
                            found_db1 = true;
                        },
                        Some("from-db2") => {
                            assert_eq!(event.database, format!("{}/testdb2", pg.host));
                            found_db2 = true;
                        },
                        _ => {},
                    }
                }
            },
            Ok(None) => break,
            Err(_) => {
                // Replication slot may not have been ready; re-insert
                if !found_db1 {
                    insert_marker(&client1, "from-db1").await;
                }
                if !found_db2 {
                    insert_marker(&client2, "from-db2").await;
                }
            },
        }
    }

    assert!(found_db1, "did not receive event from database 1 (postgres)");
    assert!(found_db2, "did not receive event from database 2 (testdb2)");

    runner1.signal_shutdown();
    runner2.signal_shutdown();
    runner1.wait().await.expect("shutdown runner1");
    runner2.wait().await.expect("shutdown runner2");
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_reconnect_resumes_events() {
    let pg = PgContainer::start_with_wal().await;
    let client = pg.setup_database().await;
    let db_cfg = pg.db_config("postgres");
    let settings = PipelineSettings::default();

    let (event_tx, mut event_rx) = mpsc::channel(1024);
    let mut runner = PipelineRunner::new(99, &db_cfg, &settings, event_tx)
        .await
        .expect("create pipeline runner");

    runner.start().await.expect("start pipeline");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify events flow before reconnect
    client
        .execute(
            "INSERT INTO test_data (col_a, col_b) VALUES ($1, $2)",
            &[&"pre-reconnect", &"row1"],
        )
        .await
        .expect("insert pre-reconnect row");

    let events = timeout(Duration::from_secs(10), event_rx.recv())
        .await
        .expect("timed out waiting for pre-reconnect event")
        .expect("channel closed");
    assert!(events.iter().any(|e| {
        e.columns
            .iter()
            .any(|c| c.value.as_deref() == Some("pre-reconnect"))
    }));

    // Stop and reconnect in-place (same runner, same event channel)
    runner.signal_shutdown();
    runner.wait().await.expect("wait after shutdown");

    runner.reconnect().await.expect("reconnect pipeline");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Insert after reconnect to verify the pipeline is live again
    client
        .execute(
            "INSERT INTO test_data (col_a, col_b) VALUES ($1, $2)",
            &[&"post-reconnect", &"row2"],
        )
        .await
        .expect("insert post-reconnect row");

    let mut found = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
        match timeout(Duration::from_secs(10), event_rx.recv()).await {
            Ok(Some(events)) => {
                if events.iter().any(|e| {
                    e.columns
                        .iter()
                        .any(|c| c.value.as_deref() == Some("post-reconnect"))
                }) {
                    found = true;
                    break;
                }
            },
            Ok(None) | Err(_) => break,
        }
    }
    assert!(found, "pipeline did not deliver events after reconnect");

    runner.signal_shutdown();
    runner.wait().await.expect("shutdown pipeline");
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test]
async fn pipeline_catches_up_after_restart_postgres_store() {
    let pg = PgContainer::start_with_wal().await;
    let client = pg.setup_database().await;
    let db_cfg = pg.db_config("postgres");
    let settings = PipelineSettings {
        store: StoreType::Postgres(PostgresStoreConfig {
            host: pg.host.clone(),
            port: pg.port,
            password: Some(SecretString::from("postgres")),
            ..Default::default()
        }),
        ..PipelineSettings::default()
    };

    test_restart_catchup(&db_cfg, &client, &settings).await;
}
