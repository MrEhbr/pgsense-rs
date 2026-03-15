use std::{collections::HashMap, sync::Arc};

use etl::{
    destination::Destination,
    error::EtlResult,
    types::{Event, TableId, TableRow},
};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, trace};

use crate::{
    events::{ColumnMeta, ScanEvent, TableMeta, extract_scan_events},
    scanner::ScanFilter,
};

/// Registry of table schemas, updated from Relation events.
pub type TableRegistry = Arc<RwLock<HashMap<TableId, TableMeta>>>;

/// Instead of writing data to an external system, extracts column values from
/// each event and forwards them as ScanEvents for rule matching.
#[derive(Clone)]
pub struct ScannerDestination {
    database: String,
    filter: ScanFilter,
    event_tx: mpsc::Sender<Vec<ScanEvent>>,
    table_registry: TableRegistry,
}

impl ScannerDestination {
    pub fn new(database: String, filter: ScanFilter, event_tx: mpsc::Sender<Vec<ScanEvent>>, table_registry: TableRegistry) -> Self {
        Self {
            database,
            filter,
            event_tx,
            table_registry,
        }
    }
}

impl Destination for ScannerDestination {
    fn name() -> &'static str {
        "pgsense-scanner"
    }

    async fn truncate_table(&self, _table_id: TableId) -> EtlResult<()> {
        Ok(())
    }

    async fn write_table_rows(&self, _table_id: TableId, _rows: Vec<TableRow>) -> EtlResult<()> {
        Ok(())
    }

    async fn write_events(&self, events: Vec<Event>) -> EtlResult<()> {
        {
            let mut registry: HashMap<TableId, TableMeta> = self.table_registry.write().await.clone();
            for event in &events {
                if let Event::Relation(rel) = event {
                    let meta = TableMeta {
                        schema: rel.table_schema.name.schema.clone(),
                        name: rel.table_schema.name.name.clone(),
                        columns: rel
                            .table_schema
                            .column_schemas
                            .iter()
                            .map(|c| ColumnMeta {
                                name: c.name.clone(),
                                type_name: c.typ.name().to_string(),
                                primary: c.primary,
                            })
                            .collect(),
                    };
                    debug!(
                        table_id = ?rel.table_schema.id,
                        schema = %meta.schema,
                        table = %meta.name,
                        columns = meta.columns.len(),
                        "registered table schema"
                    );
                    registry.insert(rel.table_schema.id, meta);
                }
            }
            *self.table_registry.write().await = registry;
        }

        let registry = self.table_registry.read().await;
        let scan_events = extract_scan_events(&events, &registry, &self.database);

        let scan_events: Vec<ScanEvent> = scan_events
            .into_iter()
            .filter(|e| self.filter.matches_schema(&e.schema_name) && self.filter.matches_table(&e.table_name))
            .map(|mut e| {
                e.columns
                    .retain(|c| self.filter.should_include_column(&c.name));
                e
            })
            .collect();

        if !scan_events.is_empty() {
            trace!(count = scan_events.len(), "forwarding scan events");
            if self.event_tx.send(scan_events).await.is_err() {
                tracing::warn!("scan event receiver dropped, events will be lost");
            }
        }

        let depth = (self.event_tx.max_capacity() - self.event_tx.capacity()) as f64;
        metrics::gauge!(crate::metrics::QUEUE_DEPTH, "database" => self.database.clone()).set(depth);

        Ok(())
    }
}
