use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result};
use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use tracing::{info, warn};

use crate::{
    alerts::dispatcher::Dispatcher,
    config::Config,
    metrics::Metrics,
    pipeline::runner::PipelineRunner,
    rules::engine::RuleEngine,
    scanner::{ScanFilter, Scanner},
    server::ServerState,
};

#[derive(Parser)]
pub struct Args {
    #[arg(long, short = 'c', value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Path to custom rules TOML file (overrides config rules_file)
    #[arg(long, short = 'r', value_name = "FILE")]
    pub rules: Option<PathBuf>,

    #[command(flatten)]
    pub verbosity: Verbosity<InfoLevel>,

    /// Pipeline identifier (used for replication slot naming)
    #[arg(long, default_value = "1")]
    pub pipeline_id: u64,
}

pub async fn run(args: Args) -> Result<()> {
    let config: Config = crate::config::load(args.config.as_deref()).context("failed to load configuration")?;
    let config = apply_overrides(&args, config);
    let _guard = crate::logging::setup(&config.log).context("failed to initialize logging")?;

    let metrics = Metrics::new();
    let ready = Arc::new(AtomicBool::new(false));

    if config.server.enabled {
        let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
        let state = ServerState {
            ready: ready.clone(),
            metrics: metrics.clone(),
        };
        tokio::spawn(async move {
            if let Err(e) = crate::server::start(addr, state).await {
                tracing::error!(error = %e, "HTTP server failed");
            }
        });
    }

    let rules_path = args
        .rules
        .as_deref()
        .or(config.rules_file.as_deref())
        .context("no rules file specified — use --rules <FILE> or set rules_file in config")?;
    let mut scanner = build_scanner(rules_path, &config.scan)?;

    let mut dispatcher = Dispatcher::from_config(&config.alerts)
        .await
        .context("failed to initialize alert dispatcher")?;
    info!(channels = dispatcher.channel_count(), "alert dispatcher ready");

    let mut runner = PipelineRunner::new(args.pipeline_id, &config.postgres, &config.pipeline)
        .await
        .context("failed to create pipeline")?;

    let mut event_rx = runner
        .take_event_receiver()
        .context("event receiver already taken")?;

    runner.start().await.context("failed to start pipeline")?;
    let mut pipeline_done = runner.spawn_wait().context("pipeline already consumed")?;
    ready.store(true, Ordering::Relaxed);

    // Watch rules file for hot reload — _watcher must stay alive for the duration
    // of the scan
    let (mut rules_rx, _watcher) = crate::watcher::watch_file(rules_path).context("failed to set up rules file watcher")?;
    let rules_path = rules_path.to_path_buf();

    let mut events_processed: u64 = 0;
    let mut findings_count: u64 = 0;

    info!("scanning started — press Ctrl+C to stop");

    loop {
        tokio::select! {
            batch = event_rx.recv() => {
                match batch {
                    Some(events) => {
                        for event in &events {
                            let timer = std::time::Instant::now();
                            let findings = scanner.scan(event);
                            let elapsed = timer.elapsed();

                            metrics.events_total.inc();
                            metrics.scan_duration_seconds.observe(elapsed.as_secs_f64());
                            events_processed += 1;

                            for finding in &findings {
                                metrics.findings_total
                                    .with_label_values(&[&finding.category, &finding.severity.to_string()])
                                    .inc();
                                findings_count += 1;
                                dispatcher.dispatch(finding).await;
                            }
                        }
                    }
                    None => {
                        warn!("event channel closed — pipeline may have stopped");
                        break;
                    }
                }
            }
            result = &mut pipeline_done => {
                match result {
                    Ok(Ok(())) => info!("pipeline exited normally"),
                    Ok(Err(e)) => warn!(error = %e, "pipeline exited with error"),
                    Err(_) => warn!("pipeline task was dropped unexpectedly"),
                }
                break;
            }
            _ = rules_rx.recv() => {
                // Debounce: let the editor finish writing before we read the file
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                // Drain any extra notifications that arrived during the delay
                while rules_rx.try_recv().is_ok() {}
                match build_scanner(&rules_path, &config.scan) {
                    Ok(new_scanner) => {
                        info!(rules = new_scanner.rule_count(), path = %rules_path.display(), "rules hot-reloaded");
                        scanner = new_scanner;
                    }
                    Err(e) => {
                        warn!(error = %e, path = %rules_path.display(), "failed to reload rules — keeping previous rules");
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("shutdown signal received");
                break;
            }
        }
    }

    dispatcher.flush().await;

    let events = events_processed;
    let findings = findings_count;
    info!(events, findings, "scan complete");

    Ok(())
}

fn build_scanner(rules_path: &Path, scan_filter: &ScanFilter) -> Result<Scanner> {
    let rules = crate::config::load_rules(rules_path).context("failed to load rules")?;
    info!(rules = rules.len(), path = %rules_path.display(), "rules loaded");
    let engine = RuleEngine::new(&rules).context("failed to compile detection rules")?;
    let scanner = Scanner::new(engine, scan_filter.clone());
    info!(rules = scanner.rule_count(), "detection engine ready");
    Ok(scanner)
}

fn apply_overrides(args: &Args, config: Config) -> Config {
    Config {
        log: crate::logging::LogConfig {
            level: args.verbosity.tracing_level().or(config.log.level),
            ..config.log
        },
        ..config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn apply_overrides_preserves_config_sections() {
        let config = Config::default();
        let args = Args {
            config: None,
            rules: None,
            verbosity: Verbosity::default(),
            pipeline_id: 1,
        };
        let result = apply_overrides(&args, config);
        assert_eq!(result.postgres.host, "localhost");
        assert_eq!(result.pipeline.batch_max_size, 1000);
        assert_eq!(result.alerts.dedup_window_seconds, 300);
        assert!(!result.server.enabled);
        assert_eq!(result.server.port, 9090);
    }
}
