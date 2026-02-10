use anyhow::Result;

use crate::{rules::config::Severity, scanner::Finding};

pub struct LogChannel;

fn format_primary_keys(pks: &[(String, String)]) -> String {
    pks.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

macro_rules! emit_finding {
    ($level:ident, $finding:expr, $pk:expr) => {
        tracing::$level!(
            rule_id = %$finding.rule_id,
            category = %$finding.category,
            severity = %$finding.severity,
            schema = %$finding.schema_name,
            table = %$finding.table_name,
            column = %$finding.column_name,
            sample = %$finding.masked_sample,
            primary_key = %$pk,
            lsn = $finding.lsn,
            "sensitive data detected"
        )
    };
}

impl LogChannel {
    pub fn send(&self, finding: &Finding) -> Result<()> {
        let pk = format_primary_keys(&finding.primary_keys);
        match finding.severity {
            Severity::Critical | Severity::High => emit_finding!(error, finding, pk),
            Severity::Medium | Severity::Low => emit_finding!(warn, finding, pk),
            Severity::Info => emit_finding!(info, finding, pk),
        }
        Ok(())
    }
}
