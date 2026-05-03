mod support;

use std::{sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use pgsense_rs::{
    alerts::dispatcher::Dispatcher,
    pipeline::{
        config::PipelineSettings,
        supervisor::{ExitSignal, PipelineStatus, database_unit::DatabaseUnit},
    },
    rules::engine::RuleEngine,
    scanner::Scanner,
};
use support::PgContainer;
use tokio::{sync::mpsc, time::timeout};

async fn test_dispatcher() -> Arc<Dispatcher> {
    Arc::new(
        Dispatcher::from_config(&pgsense_rs::alerts::config::AlertsConfig::default())
            .await
            .unwrap(),
    )
}

fn empty_scanner() -> Arc<ArcSwap<Scanner>> {
    Arc::new(ArcSwap::from_pointee(Scanner::new(RuleEngine::new(&[], false).unwrap())))
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test(flavor = "multi_thread")]
async fn unit_transitions_to_running_after_start() {
    let pg = PgContainer::start_with_wal().await;
    pg.setup_database().await;

    let config = pg.db_config("postgres");
    let (exit_tx, _exit_rx) = mpsc::channel::<ExitSignal>(1);
    let mut unit = DatabaseUnit::new(config, empty_scanner(), test_dispatcher().await);

    assert_eq!(unit.status(), PipelineStatus::Exited);

    unit.start(&PipelineSettings::default(), exit_tx)
        .await
        .expect("start unit");

    assert_eq!(unit.status(), PipelineStatus::Running);
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test(flavor = "multi_thread")]
async fn unit_transitions_to_exited_after_shutdown() {
    let pg = PgContainer::start_with_wal().await;
    pg.setup_database().await;

    let config = pg.db_config("postgres");
    let (exit_tx, mut exit_rx) = mpsc::channel::<ExitSignal>(1);
    let expected_id = config.database_id();
    let mut unit = DatabaseUnit::new(config, empty_scanner(), test_dispatcher().await);

    unit.start(&PipelineSettings::default(), exit_tx)
        .await
        .expect("start unit");

    unit.shutdown();

    let (db_id, result) = timeout(Duration::from_secs(35), exit_rx.recv())
        .await
        .expect("timed out waiting for exit signal")
        .expect("exit channel closed");

    assert_eq!(db_id, expected_id);
    assert!(result.is_ok());

    // Status may take a moment to update after the spawned task processes shutdown
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(unit.status(), PipelineStatus::Exited);
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test(flavor = "multi_thread")]
async fn unit_fails_when_pg_goes_away_and_reconnect_fails() {
    let pg = PgContainer::start_with_wal().await;
    pg.setup_database().await;

    let config = pg.db_config("postgres");
    let expected_id = config.database_id();
    let (exit_tx, mut exit_rx) = mpsc::channel::<ExitSignal>(1);
    let mut unit = DatabaseUnit::new(config, empty_scanner(), test_dispatcher().await);

    // Tighten table-error retry so the unit surfaces the failure quickly when
    // the source DB disappears. With defaults (5 attempts × 10s) the test
    // would wait ~50s for the failure to propagate.
    let settings = PipelineSettings {
        table_error_retry_delay_ms: 200,
        table_error_retry_max_attempts: 1,
        ..PipelineSettings::default()
    };

    unit.start(&settings, exit_tx).await.expect("start unit");
    assert_eq!(unit.status(), PipelineStatus::Running);

    // Kill postgres — pipeline will error, reconnect will also fail
    drop(pg);

    let (db_id, result) = timeout(Duration::from_secs(30), exit_rx.recv())
        .await
        .expect("timed out waiting for exit signal")
        .expect("exit channel closed");

    assert_eq!(db_id, expected_id);
    assert!(result.is_err());
    assert_eq!(unit.status(), PipelineStatus::Failed);
}

#[cfg_attr(not(docker), ignore = "Docker daemon not available")]
#[tokio::test(flavor = "multi_thread")]
async fn unit_reconnects_after_transient_connection_loss() {
    let pg = PgContainer::start_with_wal().await;
    let client = pg.setup_database().await;

    let config = pg.db_config("postgres");
    let (exit_tx, mut exit_rx) = mpsc::channel::<ExitSignal>(1);
    let mut unit = DatabaseUnit::new(config, empty_scanner(), test_dispatcher().await);

    unit.start(&PipelineSettings::default(), exit_tx)
        .await
        .expect("start unit");
    assert_eq!(unit.status(), PipelineStatus::Running);

    // Wait for the replication walsender to register before trying to kill it.
    // The unit's `start()` returns once workers are spawned, but the walsender
    // isn't registered until the worker actually connects.
    let mut walsender_count: i64 = 0;
    for _ in 0..30 {
        walsender_count = client
            .query_one("SELECT count(*) FROM pg_stat_replication WHERE application_name != ''", &[])
            .await
            .expect("count walsenders")
            .get(0);
        if walsender_count > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(walsender_count > 0, "expected a walsender to be registered");

    // Kill the replication walsender backend — PG stays up
    let killed: i64 = client
        .query_one(
            "SELECT count(*) FROM (
                SELECT pg_terminate_backend(pid)
                FROM pg_stat_replication
                WHERE application_name != ''
            ) t",
            &[],
        )
        .await
        .expect("terminate replication backends")
        .get(0);
    assert!(killed > 0, "expected to kill at least one walsender");

    // Give the unit time to detect failure, reconnect (1s sleep in code), and
    // settle
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Unit should still be Running — reconnect succeeded, no exit signal sent
    assert_eq!(unit.status(), PipelineStatus::Running);
    assert!(exit_rx.try_recv().is_err(), "should not have received an exit signal");

    // Verify the pipeline is live: shut down and confirm clean exit
    unit.shutdown();
    let (_, result) = timeout(Duration::from_secs(35), exit_rx.recv())
        .await
        .expect("timed out waiting for exit signal")
        .expect("exit channel closed");
    assert!(result.is_ok());
}
