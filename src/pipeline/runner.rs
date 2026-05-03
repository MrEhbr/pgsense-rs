use anyhow::{Context, Result};
use etl::{
    config::PgConnectionConfig,
    pipeline::Pipeline,
    store::{MemoryStore, PostgresStore},
};
use tokio::sync::mpsc;
use tracing::info;

use crate::{
    events::ScanEvent,
    pipeline::{
        config::{PipelineSettings, StoreType},
        destination::ScannerDestination,
        source_bootstrap,
    },
    scanner::ScanFilter,
};

enum PipelineInner {
    Memory(Pipeline<MemoryStore, ScannerDestination>),
    Postgres(Pipeline<PostgresStore, ScannerDestination>),
}

macro_rules! dispatch {
    ($pipeline:expr, $method:ident) => {
        match $pipeline {
            PipelineInner::Memory(p) => p.$method(),
            PipelineInner::Postgres(p) => p.$method(),
        }
    };
    (await $pipeline:expr, $method:ident) => {
        match $pipeline {
            PipelineInner::Memory(p) => p.$method().await,
            PipelineInner::Postgres(p) => p.$method().await,
        }
    };
}

impl PipelineInner {
    async fn start(&mut self) -> Result<()> {
        dispatch!(await self, start).map_err(|e| anyhow::anyhow!("pipeline start failed: {e}"))
    }

    async fn wait(self) -> Result<()> {
        dispatch!(await self, wait).map_err(|e| anyhow::anyhow!("pipeline error: {e}"))
    }

    fn shutdown(&self) {
        dispatch!(self, shutdown);
    }

    async fn shutdown_and_wait(self) -> Result<()> {
        dispatch!(await self, shutdown_and_wait).map_err(|e| anyhow::anyhow!("pipeline shutdown failed: {e}"))
    }
}

pub struct PipelineRunner {
    pipeline: Option<PipelineInner>,
    pipeline_id: u64,
    database: String,
    scan_filter: ScanFilter,
    pg_connection: PgConnectionConfig,
    publication: String,
    settings: PipelineSettings,
    event_tx: mpsc::Sender<Vec<ScanEvent>>,
}

impl PipelineRunner {
    #[tracing::instrument(skip_all, fields(pipeline_id))]
    pub async fn new(
        pipeline_id: u64,
        db: &crate::pipeline::config::DatabaseConfig,
        pipeline_settings: &PipelineSettings,
        event_tx: mpsc::Sender<Vec<ScanEvent>>,
    ) -> Result<Self> {
        let database = db.database_id();
        let scan_filter = db.scan.clone().unwrap_or_default();

        if !scan_filter.include_schemas.is_empty() || !scan_filter.exclude_tables.is_empty() || !scan_filter.exclude_columns.is_empty() {
            info!(
                include_schemas = ?scan_filter.include_schemas,
                exclude_tables = ?scan_filter.exclude_tables,
                exclude_columns = ?scan_filter.exclude_columns,
                "scan filter active"
            );
        }

        let pg_connection = db.to_pg_connection_config();

        // The etl pipeline calls `etl.describe_table_schema(...)` and
        // `etl.describe_table_identity(...)` against the source DB during
        // table sync. With Postgres store, etl's own migrations install
        // these; with Memory store nothing else does, so install them here.
        if pipeline_settings.store == StoreType::Memory {
            source_bootstrap::apply(&pg_connection)
                .await
                .context("source database bootstrap failed")?;
        }

        let mut runner = Self {
            pipeline: None,
            pipeline_id,
            database,
            scan_filter,
            pg_connection,
            publication: db.publication.clone(),
            settings: pipeline_settings.clone(),
            event_tx,
        };

        runner.build_pipeline().await?;
        Ok(runner)
    }

    async fn build_pipeline(&mut self) -> Result<()> {
        let pipeline_config = self
            .settings
            .to_pipeline_config(self.pipeline_id, &self.publication, self.pg_connection.clone())
            .context("failed to build pipeline config")?;

        let destination = ScannerDestination::new(self.database.clone(), self.scan_filter.clone(), self.event_tx.clone());

        let pipeline = match &self.settings.store {
            StoreType::Memory => {
                info!("using in-memory store (state lost on restart)");
                let store = MemoryStore::new();
                PipelineInner::Memory(Pipeline::new(pipeline_config, store, destination))
            },
            StoreType::Postgres => {
                info!("using PostgreSQL store (state persisted in source DB under `etl` schema)");
                let store = PostgresStore::new(self.pipeline_id, self.pg_connection.clone())
                    .await
                    .map_err(|e| anyhow::anyhow!("Postgres store init failed: {e}"))?;
                PipelineInner::Postgres(Pipeline::new(pipeline_config, store, destination))
            },
        };

        self.pipeline = Some(pipeline);
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub async fn start(&mut self) -> Result<()> {
        let pipeline = self
            .pipeline
            .as_mut()
            .context("pipeline already consumed")?;
        info!("starting replication pipeline");
        pipeline.start().await?;
        info!("replication pipeline started");
        Ok(())
    }

    /// Wait for the pipeline to complete. After wait returns, the runner can
    /// be reconnected via [`reconnect`].
    pub async fn wait(&mut self) -> Result<()> {
        let pipeline = self.pipeline.take().context("pipeline already consumed")?;
        pipeline.wait().await
    }

    /// Rebuild the pipeline from stored params and start it.
    pub async fn reconnect(&mut self) -> Result<()> {
        self.build_pipeline().await?;
        self.start().await
    }

    /// Signal the pipeline to stop gracefully. Non-consuming — the running
    /// `wait()` call will return once workers finish.
    pub fn signal_shutdown(&self) {
        if let Some(pipeline) = &self.pipeline {
            pipeline.shutdown();
        }
    }

    pub async fn shutdown(mut self) -> Result<()> {
        let pipeline = self.pipeline.take().context("pipeline already consumed")?;
        pipeline.shutdown_and_wait().await
    }
}
