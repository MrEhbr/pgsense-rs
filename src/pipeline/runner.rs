use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use etl::{pipeline::Pipeline, store::both::memory::MemoryStore};
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::info;

use crate::{
    events::ScanEvent,
    pipeline::{
        config::{PipelineSettings, PostgresConfig, StoreType},
        destination::{ScannerDestination, TableRegistry},
        store::{PostgresStore, SqliteStore},
    },
};

const EVENT_CHANNEL_SIZE: usize = 1024;

enum PipelineInner {
    Memory(Pipeline<MemoryStore, ScannerDestination>),
    Postgres(Pipeline<PostgresStore, ScannerDestination>),
    Sqlite(Pipeline<SqliteStore, ScannerDestination>),
}

pub struct PipelineRunner {
    pipeline: Option<PipelineInner>,
    event_rx: Option<mpsc::Receiver<Vec<ScanEvent>>>,
    table_registry: TableRegistry,
}

impl PipelineRunner {
    pub async fn new(pipeline_id: u64, postgres_config: &PostgresConfig, pipeline_settings: &PipelineSettings) -> Result<Self> {
        let pg_connection = postgres_config.to_pg_connection_config();
        let pipeline_config = pipeline_settings
            .to_pipeline_config(pipeline_id, &postgres_config.publication, pg_connection.clone())
            .context("failed to build pipeline config")?;

        let table_registry: TableRegistry = Arc::new(RwLock::new(HashMap::new()));
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_SIZE);
        let destination = ScannerDestination::new(event_tx, table_registry.clone());

        let pipeline = match &pipeline_settings.store {
            StoreType::Memory => {
                info!("using in-memory store (state lost on restart)");
                let store = MemoryStore::new();
                PipelineInner::Memory(Pipeline::new(pipeline_config, store, destination))
            },
            StoreType::Postgres(pg_store_config) => {
                info!(schema = %pg_store_config.schema, "using PostgreSQL store (persistent state)");
                let store = PostgresStore::new(pipeline_id, pg_store_config)
                    .await
                    .map_err(|e| anyhow::anyhow!("Postgres store init failed: {e}"))?;
                PipelineInner::Postgres(Pipeline::new(pipeline_config, store, destination))
            },
            StoreType::Sqlite(sqlite_config) => {
                info!(path = %sqlite_config.path, "using SQLite store (persistent local file)");
                let store = SqliteStore::new(pipeline_id, &sqlite_config.path)
                    .await
                    .map_err(|e| anyhow::anyhow!("SQLite store init failed: {e}"))?;
                PipelineInner::Sqlite(Pipeline::new(pipeline_config, store, destination))
            },
        };

        Ok(Self {
            pipeline: Some(pipeline),
            event_rx: Some(event_rx),
            table_registry,
        })
    }

    /// Connects to PostgreSQL and begins replication streaming.
    pub async fn start(&mut self) -> Result<()> {
        let pipeline = self
            .pipeline
            .as_mut()
            .context("pipeline already consumed")?;
        info!("starting replication pipeline");
        match pipeline {
            PipelineInner::Memory(p) => p
                .start()
                .await
                .map_err(|e| anyhow::anyhow!("pipeline start failed: {e}"))?,
            PipelineInner::Postgres(p) => p
                .start()
                .await
                .map_err(|e| anyhow::anyhow!("pipeline start failed: {e}"))?,
            PipelineInner::Sqlite(p) => p
                .start()
                .await
                .map_err(|e| anyhow::anyhow!("pipeline start failed: {e}"))?,
        }
        info!("replication pipeline started");
        Ok(())
    }

    /// Wait for the pipeline to complete (consumes the pipeline).
    pub async fn wait(mut self) -> Result<()> {
        let pipeline = self.pipeline.take().context("pipeline already consumed")?;
        match pipeline {
            PipelineInner::Memory(p) => p
                .wait()
                .await
                .map_err(|e| anyhow::anyhow!("pipeline error: {e}"))?,
            PipelineInner::Postgres(p) => p
                .wait()
                .await
                .map_err(|e| anyhow::anyhow!("pipeline error: {e}"))?,
            PipelineInner::Sqlite(p) => p
                .wait()
                .await
                .map_err(|e| anyhow::anyhow!("pipeline error: {e}"))?,
        }
        Ok(())
    }

    /// Gracefully shut down the pipeline and wait for all workers to finish.
    pub async fn shutdown(mut self) -> Result<()> {
        let pipeline = self.pipeline.take().context("pipeline already consumed")?;
        match pipeline {
            PipelineInner::Memory(p) => p
                .shutdown_and_wait()
                .await
                .map_err(|e| anyhow::anyhow!("pipeline shutdown failed: {e}"))?,
            PipelineInner::Postgres(p) => p
                .shutdown_and_wait()
                .await
                .map_err(|e| anyhow::anyhow!("pipeline shutdown failed: {e}"))?,
            PipelineInner::Sqlite(p) => p
                .shutdown_and_wait()
                .await
                .map_err(|e| anyhow::anyhow!("pipeline shutdown failed: {e}"))?,
        }
        Ok(())
    }

    /// Spawn a background task that waits for the pipeline to finish.
    /// Returns a oneshot receiver that resolves when the pipeline exits
    /// (either normally or due to a connection error).
    /// Must be called after `start()`.
    pub fn spawn_wait(&mut self) -> Option<oneshot::Receiver<Result<()>>> {
        let pipeline = self.pipeline.take()?;
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = match pipeline {
                PipelineInner::Memory(p) => p
                    .wait()
                    .await
                    .map_err(|e| anyhow::anyhow!("pipeline error: {e}")),
                PipelineInner::Postgres(p) => p
                    .wait()
                    .await
                    .map_err(|e| anyhow::anyhow!("pipeline error: {e}")),
                PipelineInner::Sqlite(p) => p
                    .wait()
                    .await
                    .map_err(|e| anyhow::anyhow!("pipeline error: {e}")),
            };
            let _ = tx.send(result);
        });
        Some(rx)
    }

    /// Can only be called once.
    pub fn take_event_receiver(&mut self) -> Option<mpsc::Receiver<Vec<ScanEvent>>> {
        self.event_rx.take()
    }

    pub fn table_registry(&self) -> &TableRegistry {
        &self.table_registry
    }
}
