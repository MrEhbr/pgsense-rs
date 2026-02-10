use anyhow::Result;

use super::AlertPayload;
use crate::scanner::Finding;

pub struct StdoutChannel;

impl StdoutChannel {
    pub fn send(&self, finding: &Finding) -> Result<()> {
        let payload = AlertPayload::from(finding);
        let json = serde_json::to_string(&payload)?;
        println!("{json}");
        Ok(())
    }
}
