use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use tracing::{info, warn};

use crate::{
    alerts::dispatcher::Dispatcher,
    config::Config,
    metrics,
    pipeline::supervisor::Supervisor,
    rules::{config::RuleConfig, engine::RuleEngine},
    scanner::Scanner,
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
}

pub async fn run(args: Args) -> Result<()> {
    let config: Config = crate::config::load(args.config.as_deref()).context("failed to load configuration")?;
    let config = apply_overrides(&args, config);
    config.validate().context("invalid configuration")?;
    let _guard = crate::logging::setup(&config.log).context("failed to initialize logging")?;

    metrics::init();
    let ready = Arc::new(AtomicBool::new(false));

    if config.server.enabled {
        let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
        let state = ServerState { ready: ready.clone() };
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
    let (scanner, rules) = build_scanner(rules_path)?;
    metrics::RULES_LOADED.set(scanner.rule_count() as i64);
    let scanner = Arc::new(ArcSwap::from_pointee(scanner));

    let dispatcher = Dispatcher::from_config(&config.alerts)
        .await
        .context("failed to initialize alert dispatcher")?;
    dispatcher.validate_channel_routing(&rules);
    info!(channels = dispatcher.channel_count(), "alert dispatcher ready");
    let dispatcher = Arc::new(dispatcher);

    let databases = config.databases();
    let (mut supervisor, mut exit_rx) = Supervisor::new(databases, config.pipeline.clone(), scanner.clone(), dispatcher.clone());
    supervisor.start().await?;

    // Watch rules file for hot reload — _watcher must stay alive for scan duration
    let (mut rules_rx, _watcher) = crate::watcher::watch_file(rules_path).context("failed to set up rules file watcher")?;
    let rules_path = rules_path.to_path_buf();

    ready.store(true, Ordering::Relaxed);

    info!(
        databases = supervisor.database_count(),
        "scanning started — press Ctrl+C to stop"
    );

    loop {
        tokio::select! {
            Some((database, result)) = exit_rx.recv() => {
                match &result {
                    Ok(()) => info!(database = %database, "pipeline exited normally"),
                    Err(e) => warn!(database = %database, error = %e, "pipeline exited with error"),
                }
                if supervisor.all_terminated() {
                    break;
                }
            }
            _ = rules_rx.recv() => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                while rules_rx.try_recv().is_ok() {}
                match build_scanner(&rules_path) {
                    Ok((new_scanner, new_rules)) => {
                        dispatcher.validate_channel_routing(&new_rules);
                        metrics::RULES_LOADED.set(new_scanner.rule_count() as i64);
                        metrics::CONFIG_RELOADS.with_label_values(&["ok"]).inc();
                        info!(rules = new_scanner.rule_count(), path = %rules_path.display(), "rules hot-reloaded");
                        scanner.store(Arc::new(new_scanner));
                    }
                    Err(e) => {
                        metrics::CONFIG_RELOADS.with_label_values(&["error"]).inc();
                        warn!(error = %e, path = %rules_path.display(), "failed to reload rules — keeping previous rules");
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("shutdown signal received, waiting for pipelines to stop...");
                supervisor.shutdown();
            }
        }
    }

    dispatcher.flush().await;

    info!("scan complete");

    Ok(())
}

fn build_scanner(rules_path: &Path) -> Result<(Scanner, Vec<RuleConfig>)> {
    let rules = crate::config::load_rules(rules_path).context("failed to load rules")?;
    let start = std::time::Instant::now();
    let engine = RuleEngine::new(&rules).context("failed to compile detection rules")?;
    let elapsed = start.elapsed();
    let scanner = Scanner::new(engine);

    let (regex, builtin, script) = rules
        .iter()
        .fold((0u32, 0u32, 0u32), |(r, b, s), rule| match rule.rule_type {
            crate::rules::config::RuleType::Regex => (r + 1, b, s),
            crate::rules::config::RuleType::Builtin => (r, b + 1, s),
            crate::rules::config::RuleType::Script => (r, b, s + 1),
        });
    info!(
        total = scanner.rule_count(),
        regex,
        builtin,
        script,
        compile_ms = elapsed.as_millis(),
        path = %rules_path.display(),
        "detection engine ready"
    );
    Ok((scanner, rules))
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
        };
        let result = apply_overrides(&args, config);
        assert_eq!(result.pipeline.batch_max_size, 1000);
        assert_eq!(result.alerts.dedup_window_seconds, 300);
        assert!(!result.server.enabled);
        assert_eq!(result.server.port, 9090);
    }
}
