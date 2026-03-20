use std::collections::HashMap;

use anyhow::{Context, Result};
use aws_sdk_secretsmanager::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct SecretsReader {
    client: Client,
    cache: OnceCell<HashMap<String, Value>>,
}

impl SecretsReader {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            cache: OnceCell::const_new(),
        }
    }

    async fn load_all(&self, secret_id: &str) -> Result<HashMap<String, Value>> {
        let response = self
            .client
            .get_secret_value()
            .secret_id(secret_id)
            .send()
            .await
            .context("Failed to retrieve secret value from Secrets Manager")?;

        let secret_str = response
            .secret_string()
            .context("Secret value is missing or not a string")?;

        serde_json::from_str(secret_str).context("Failed to parse secret string as JSON")
    }

    async fn get_map(&self, secret_id: &str) -> Result<&HashMap<String, Value>> {
        self.cache
            .get_or_try_init(|| async { self.load_all(secret_id).await })
            .await
    }

    pub async fn get_string(&self, secret_id: &str, key: &str) -> Result<String> {
        let response = self
            .client
            .get_secret_value()
            .secret_id(secret_id)
            .send()
            .await?;

        let secret_str = response
            .secret_string()
            .ok_or_else(|| anyhow::anyhow!("Missing secret string"))?;

        let parsed: serde_json::Value = serde_json::from_str(secret_str)?;

        if let Some(val) = parsed.get(key).and_then(|v| v.as_str()) {
            return Ok(val.to_string());
        }

        if let Some(inner) = parsed.as_str() {
            let inner_json: serde_json::Value = serde_json::from_str(inner)?;
            if let Some(val) = inner_json.get(key).and_then(|v| v.as_str()) {
                return Ok(val.to_string());
            }
        }

        Err(anyhow::anyhow!("Key '{}' not found in secret", key))
    }

    pub async fn get_typed<T: DeserializeOwned>(&self, secret_id: &str, key: &str) -> Result<T> {
        let map = self.get_map(secret_id).await?;

        let value = map
            .get(key)
            .context(format!("Key '{}' not found in secret", key))?;

        serde_json::from_value(value.clone()).context("Failed to deserialize secret value")
    }
}
