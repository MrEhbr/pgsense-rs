use anyhow::Result;

use crate::{rules::config::Severity, scanner::Finding};

pub struct LogChannel;

fn format_primary_keys(pks: &[(String, String)]) -> String {
    use std::fmt::Write;
    let mut buf = String::new();
    for (i, (k, v)) in pks.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        let _ = write!(buf, "{k}={v}");
    }
    buf
}

macro_rules! emit_finding {
    ($level:ident, $finding:expr, $pk:expr) => {
        tracing::$level!(
            database = %$finding.database,
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
