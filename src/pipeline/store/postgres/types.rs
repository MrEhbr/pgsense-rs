use etl::{
    error::{ErrorKind, EtlResult},
    etl_error,
    state::table::{RetryPolicy, TableReplicationPhase},
};
use etl_postgres::replication::state::TableReplicationState;
use sqlx::{FromRow, postgres::types::Oid as SqlxTableId};

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub(super) struct TableReplicationStateRow {
    pub id: i64,
    pub pipeline_id: i64,
    pub table_id: SqlxTableId,
    pub state: String,
    pub metadata: Option<serde_json::Value>,
    pub prev: Option<i64>,
    pub is_current: bool,
}

impl TableReplicationStateRow {
    pub(super) fn deserialize_metadata(&self) -> EtlResult<TableReplicationState> {
        let metadata = self
            .metadata
            .as_ref()
            .ok_or_else(|| etl_error!(ErrorKind::DeserializationError, "Missing metadata in state row"))?;
        serde_json::from_value(metadata.clone()).map_err(|e| etl_error!(ErrorKind::DeserializationError, "State deserialization failed", e.to_string()))
    }
}

impl TryFrom<TableReplicationStateRow> for TableReplicationPhase {
    type Error = etl::error::EtlError;

    fn try_from(row: TableReplicationStateRow) -> EtlResult<Self> {
        let state = row.deserialize_metadata()?;
        match state {
            TableReplicationState::Init => Ok(TableReplicationPhase::Init),
            TableReplicationState::DataSync => Ok(TableReplicationPhase::DataSync),
            TableReplicationState::FinishedCopy => Ok(TableReplicationPhase::FinishedCopy),
            TableReplicationState::SyncDone { lsn } => Ok(TableReplicationPhase::SyncDone { lsn }),
            TableReplicationState::Ready => Ok(TableReplicationPhase::Ready),
            TableReplicationState::Errored {
                reason,
                solution,
                retry_policy,
            } => {
                let etl_retry = match retry_policy {
                    etl_postgres::replication::state::RetryPolicy::NoRetry => RetryPolicy::NoRetry,
                    etl_postgres::replication::state::RetryPolicy::ManualRetry => RetryPolicy::ManualRetry,
                    etl_postgres::replication::state::RetryPolicy::TimedRetry { next_retry } => RetryPolicy::TimedRetry { next_retry },
                };
                Ok(TableReplicationPhase::Errored {
                    reason,
                    solution,
                    retry_policy: etl_retry,
                })
            },
        }
    }
}
