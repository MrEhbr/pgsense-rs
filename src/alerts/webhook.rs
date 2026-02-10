use std::time::Duration;

use anyhow::{Context, Result};

use super::{AlertPayload, config::WebhookConfig};
use crate::scanner::Finding;

pub struct WebhookChannel {
    client: reqwest::Client,
    url: String,
}

impl WebhookChannel {
    pub fn new(config: &WebhookConfig) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in &config.headers {
            let name = key
                .parse::<reqwest::header::HeaderName>()
                .with_context(|| format!("invalid header name: {key}"))?;
            let val = value
                .parse::<reqwest::header::HeaderValue>()
                .with_context(|| format!("invalid header value for {key}"))?;
            headers.insert(name, val);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .default_headers(headers)
            .build()
            .context("failed to build webhook HTTP client")?;

        Ok(Self {
            client,
            url: config.url.clone(),
        })
    }

    pub async fn send(&self, finding: &Finding) -> Result<()> {
        let payload = AlertPayload::from(finding);
        let response = self
            .client
            .post(&self.url)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("webhook POST to {} failed", self.url))?;

        if !response.status().is_success() {
            anyhow::bail!("webhook {} returned status {}", self.url, response.status());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn webhook_channel_builds_from_config() {
        let config = WebhookConfig {
            url: "https://hooks.example.com/alert".to_string(),
            headers: HashMap::from([("Authorization".to_string(), "Bearer tok".to_string())]),
            timeout_ms: 3000,
        };
        let channel = WebhookChannel::new(&config).unwrap();
        assert_eq!(channel.url, "https://hooks.example.com/alert");
    }

    #[test]
    fn webhook_rejects_invalid_header() {
        let config = WebhookConfig {
            url: "https://hooks.example.com".to_string(),
            headers: HashMap::from([("Invalid\nHeader".to_string(), "value".to_string())]),
            timeout_ms: 5000,
        };
        assert!(WebhookChannel::new(&config).is_err());
    }
}
