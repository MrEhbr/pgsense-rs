use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    sync::Mutex,
};

use anyhow::{Context, Result};

use super::{AlertPayload, config::JsonlAlertConfig};
use crate::scanner::Finding;

pub struct JsonlChannel {
    writer: Mutex<BufWriter<File>>,
}

impl JsonlChannel {
    pub fn new(config: &JsonlAlertConfig) -> Result<Self> {
        if let Some(parent) = config.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).with_context(|| format!("failed to create directory: {}", parent.display()))?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.path)
            .with_context(|| format!("failed to open JSONL file: {}", config.path.display()))?;

        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
        })
    }

    pub fn send(&self, finding: &Finding) -> Result<()> {
        let payload = AlertPayload::from(finding);
        let mut line = serde_json::to_string(&payload)?;
        line.push('\n');

        let mut writer = self.writer.lock().expect("jsonl writer lock poisoned");
        writer
            .write_all(line.as_bytes())
            .context("failed to write JSONL line")?;
        writer.flush().context("failed to flush JSONL writer")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufRead;

    use super::*;
    use crate::rules::config::Severity;

    fn test_finding() -> Finding {
        Finding {
            rule_id: "test-rule".to_string(),
            description: "test description".to_string(),
            category: "test".to_string(),
            severity: Severity::Medium,
            schema_name: "public".to_string(),
            table_name: "events".to_string(),
            column_name: "data".to_string(),
            masked_sample: "***masked***".to_string(),
            value_hash: 0,
            primary_keys: vec![("id".to_string(), "1".to_string())],
            lsn: 1,
        }
    }

    #[test]
    fn writes_valid_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");

        let config = JsonlAlertConfig {
            enabled: true,
            path: path.clone(),
        };

        let channel = JsonlChannel::new(&config).unwrap();
        channel.send(&test_finding()).unwrap();
        channel.send(&test_finding()).unwrap();

        let file = File::open(&path).unwrap();
        let lines: Vec<String> = std::io::BufReader::new(file)
            .lines()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(lines.len(), 2);
        for line in &lines {
            let payload: AlertPayload = serde_json::from_str(line).unwrap();
            assert_eq!(payload.rule_id, "test-rule");
            assert_eq!(payload.severity, "MEDIUM");
            assert_eq!(payload.table_name, "events");
        }
    }

    #[test]
    fn creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("alerts.jsonl");

        let config = JsonlAlertConfig {
            enabled: true,
            path: path.clone(),
        };

        let channel = JsonlChannel::new(&config).unwrap();
        channel.send(&test_finding()).unwrap();
        assert!(path.exists());
    }
}
