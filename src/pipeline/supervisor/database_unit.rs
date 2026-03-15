use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, bail};
use arc_swap::ArcSwap;
use tokio::sync::{mpsc, watch};
use tracing::{info, warn};

use super::{ExitSignal, PipelineStatus};
use crate::{
    alerts::dispatcher::Dispatcher,
    events::ScanEvent,
    pipeline::{
        config::{DatabaseConfig, PipelineSettings},
        runner::PipelineRunner,
    },
    scanner::Scanner,
};

pub struct DatabaseUnit {
    config: DatabaseConfig,
    database_id: String,
    status: Arc<AtomicU8>,
    scanner: Arc<ArcSwap<Scanner>>,
    dispatcher: Arc<Dispatcher>,
    shutdown_tx: Option<watch::Sender<()>>,
}

impl DatabaseUnit {
    pub fn new(config: DatabaseConfig, scanner: Arc<ArcSwap<Scanner>>, dispatcher: Arc<Dispatcher>) -> Self {
        let database_id = config.database_id();
        Self {
            config,
            database_id,
            status: Arc::new(AtomicU8::new(PipelineStatus::Exited as u8)),
            scanner,
            dispatcher,
            shutdown_tx: None,
        }
    }

    pub fn status(&self) -> PipelineStatus {
        PipelineStatus::from_u8(self.status.load(Ordering::Relaxed))
    }

    pub fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
        }
    }

    pub async fn start(&mut self, pipeline_settings: &PipelineSettings, exit_tx: mpsc::Sender<ExitSignal>) -> Result<()> {
        if self.shutdown_tx.is_some() {
            bail!("unit for {} is already running", self.database_id);
        }

        let pipeline_id = self.config.pipeline_id();
        let (event_tx, event_rx) = mpsc::channel::<Vec<ScanEvent>>(256);

        let mut runner = PipelineRunner::new(pipeline_id, &self.config, pipeline_settings, event_tx)
            .await
            .with_context(|| format!("failed to create pipeline for {}", self.database_id))?;

        runner
            .start()
            .await
            .with_context(|| format!("failed to start pipeline for {}", self.database_id))?;

        let scanner = self.scanner.clone();
        let dispatcher = self.dispatcher.clone();
        tokio::spawn(async move {
            Self::scan_loop(event_rx, &scanner, &dispatcher).await;
        });

        let (shutdown_tx, mut shutdown_rx) = watch::channel(());
        self.shutdown_tx = Some(shutdown_tx);
        self.status
            .store(PipelineStatus::Running as u8, Ordering::Relaxed);

        let status = self.status.clone();
        let database = self.database_id.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = runner.wait() => {
                        match result {
                            Ok(()) => {
                                status.store(PipelineStatus::Exited as u8, Ordering::Relaxed);
                                info!(database = %database, "pipeline exited normally");
                                let _ = exit_tx.send((database, Ok(()))).await;
                                return;
                            },
                            Err(e) => {
                                warn!(database = %database, error = %e, "pipeline failed, attempting reconnect");
                                match runner.reconnect().await {
                                    Ok(()) => {
                                        info!(database = %database, "pipeline reconnected");
                                        tokio::time::sleep(Duration::from_secs(1)).await;
                                        continue;
                                    },
                                    Err(reconnect_err) => {
                                        status.store(PipelineStatus::Failed as u8, Ordering::Relaxed);
                                        let _ = exit_tx.send((database, Err(reconnect_err))).await;
                                        return;
                                    },
                                }
                            },
                        }
                    }
                    result = shutdown_rx.changed() => {
                        if result.is_err() {
                            // Sender dropped without explicit signal — abnormal exit
                            warn!(database = %database, "shutdown channel closed unexpectedly");
                            status.store(PipelineStatus::Exited as u8, Ordering::Relaxed);
                            return;
                        }
                        runner.signal_shutdown();
                        match tokio::time::timeout(Duration::from_secs(30), runner.wait()).await {
                            Ok(Err(e)) => warn!(database = %database, error = %e, "pipeline stopped with error during shutdown"),
                            Err(_) => warn!(database = %database, "pipeline shutdown timed out after 30s"),
                            Ok(Ok(())) => {}
                        }
                        status.store(PipelineStatus::Exited as u8, Ordering::Relaxed);
                        info!(database = %database, "pipeline shut down");
                        let _ = exit_tx.send((database, Ok(()))).await;
                        return;
                    }
                }
            }
        });

        info!(database = %self.database_id, pipeline_id, "pipeline started");
        Ok(())
    }

    async fn scan_loop(mut event_rx: mpsc::Receiver<Vec<ScanEvent>>, scanner: &ArcSwap<Scanner>, dispatcher: &Dispatcher) {
        while let Some(events) = event_rx.recv().await {
            let scanner = scanner.load();
            let batch_len = events.len();

            let mut db = String::new();
            for event in &events {
                let start = std::time::Instant::now();
                db.clone_from(&event.database);
                metrics::counter!(crate::metrics::EVENTS_TOTAL, "database" => event.database.clone()).increment(1);

                let findings = scanner.scan(event);
                for finding in &findings {
                    metrics::counter!(
                        crate::metrics::FINDINGS_TOTAL,
                        "database" => finding.database.clone(),
                        "category" => finding.category.clone(),
                        "severity" => finding.severity.to_string(),
                    )
                    .increment(1);
                }

                metrics::histogram!(crate::metrics::SCAN_DURATION, "database" => event.database.clone()).record(start.elapsed());

                if !findings.is_empty() {
                    let dispatch_start = std::time::Instant::now();
                    for finding in &findings {
                        dispatcher.dispatch(finding).await;
                    }
                    metrics::histogram!(crate::metrics::DISPATCH_DURATION, "database" => event.database.clone()).record(dispatch_start.elapsed());
                }
            }
            metrics::histogram!(crate::metrics::BATCH_SIZE, "database" => db).record(batch_len as f64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_unit() -> DatabaseUnit {
        let scanner = Arc::new(ArcSwap::from_pointee(Scanner::new(
            crate::rules::engine::RuleEngine::new(&[]).unwrap(),
        )));
        let dispatcher = Arc::new(Dispatcher::default_for_test());
        DatabaseUnit::new(DatabaseConfig::default(), scanner, dispatcher)
    }

    #[test]
    fn new_unit_starts_as_exited() {
        let unit = make_unit();
        assert_eq!(unit.status(), PipelineStatus::Exited);
    }

    #[test]
    fn shutdown_before_start_is_noop() {
        let unit = make_unit();
        unit.shutdown();
        assert_eq!(unit.status(), PipelineStatus::Exited);
    }
}
