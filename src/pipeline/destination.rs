use etl::{
    destination::{
        Destination,
        async_result::{TruncateTableResult, WriteEventsResult, WriteTableRowsResult},
    },
    error::EtlResult,
    types::{Event, ReplicatedTableSchema, TableRow},
};
use tokio::sync::mpsc;
use tracing::trace;

use crate::{
    events::{ScanEvent, extract_scan_events},
    metrics,
    scanner::ScanFilter,
};

/// Instead of writing data to an external system, extracts column values from
/// each event and forwards them as ScanEvents for rule matching.
#[derive(Clone)]
pub struct ScannerDestination {
    database: String,
    filter: ScanFilter,
    event_tx: mpsc::Sender<Vec<ScanEvent>>,
}

impl ScannerDestination {
    pub fn new(database: String, filter: ScanFilter, event_tx: mpsc::Sender<Vec<ScanEvent>>) -> Self {
        Self { database, filter, event_tx }
    }
}

impl Destination for ScannerDestination {
    fn name() -> &'static str {
        "pgsense-scanner"
    }

    async fn truncate_table(&self, _replicated_table_schema: &ReplicatedTableSchema, async_result: TruncateTableResult<()>) -> EtlResult<()> {
        async_result.send(Ok(()));
        Ok(())
    }

    async fn write_table_rows(
        &self,
        _replicated_table_schema: &ReplicatedTableSchema,
        _table_rows: Vec<TableRow>,
        async_result: WriteTableRowsResult<()>,
    ) -> EtlResult<()> {
        async_result.send(Ok(()));
        Ok(())
    }

    async fn write_events(&self, events: Vec<Event>, async_result: WriteEventsResult<()>) -> EtlResult<()> {
        let scan_events = extract_scan_events(&events, &self.database);

        let scan_events: Vec<ScanEvent> = scan_events
            .into_iter()
            .filter(|e| {
                if !self.filter.matches_schema(&e.schema_name) {
                    metrics::EVENTS_SKIPPED
                        .with_label_values(&[&self.database, "schema_excluded"])
                        .inc();
                    return false;
                }
                if !self.filter.matches_table(&e.table_name) {
                    metrics::EVENTS_SKIPPED
                        .with_label_values(&[&self.database, "table_excluded"])
                        .inc();
                    return false;
                }
                true
            })
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

        let depth = (self.event_tx.max_capacity() - self.event_tx.capacity()) as i64;
        metrics::QUEUE_DEPTH
            .with_label_values(&[&self.database])
            .set(depth);

        async_result.send(Ok(()));
        Ok(())
    }
}
