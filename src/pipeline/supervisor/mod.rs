pub mod database_unit;

use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use arc_swap::ArcSwap;
use database_unit::DatabaseUnit;
use tokio::sync::mpsc;

use crate::{
    alerts::dispatcher::Dispatcher,
    pipeline::config::{DatabaseConfig, PipelineSettings},
    scanner::Scanner,
};

pub type ExitSignal = (String, Result<()>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PipelineStatus {
    Exited = 0,
    Running = 1,
    Failed = 2,
}

impl PipelineStatus {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Running,
            2 => Self::Failed,
            _ => Self::Exited,
        }
    }
}

pub struct Supervisor {
    units: HashMap<String, DatabaseUnit>,
    pipeline_settings: PipelineSettings,
    exit_tx: mpsc::Sender<ExitSignal>,
}

impl Supervisor {
    pub fn new(
        databases: Vec<DatabaseConfig>,
        pipeline_settings: PipelineSettings,
        scanner: Arc<ArcSwap<Scanner>>,
        dispatcher: Arc<Dispatcher>,
    ) -> (Self, mpsc::Receiver<ExitSignal>) {
        let (exit_tx, exit_rx) = mpsc::channel::<ExitSignal>(databases.len().max(1));

        let units = databases
            .into_iter()
            .map(|config| {
                let id = config.database_id();
                let unit = DatabaseUnit::new(config, scanner.clone(), dispatcher.clone());
                (id, unit)
            })
            .collect();

        let supervisor = Self {
            units,
            pipeline_settings,
            exit_tx,
        };

        (supervisor, exit_rx)
    }

    pub async fn start(&mut self) -> Result<()> {
        for unit in self.units.values_mut() {
            unit.start(&self.pipeline_settings, self.exit_tx.clone())
                .await?;
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        for unit in self.units.values() {
            unit.shutdown();
        }
    }

    pub fn all_terminated(&self) -> bool {
        self.units
            .values()
            .all(|u| matches!(u.status(), PipelineStatus::Exited | PipelineStatus::Failed))
    }

    pub fn database_count(&self) -> usize {
        self.units.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_supervisor() -> (Supervisor, mpsc::Receiver<ExitSignal>) {
        let scanner = Arc::new(ArcSwap::from_pointee(Scanner::new(
            crate::rules::engine::RuleEngine::new(&[]).unwrap(),
        )));
        let dispatcher = Arc::new(Dispatcher::default_for_test());
        let databases = vec![
            DatabaseConfig {
                dbname: "db1".into(),
                ..Default::default()
            },
            DatabaseConfig {
                dbname: "db2".into(),
                ..Default::default()
            },
        ];
        Supervisor::new(databases, PipelineSettings::default(), scanner, dispatcher)
    }

    #[test]
    fn database_count_matches_config() {
        let (sup, _exit_rx) = make_supervisor();
        assert_eq!(sup.database_count(), 2);
    }
}
