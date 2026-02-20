use anyhow::{Context, Result};
use aws_sdk_secretsmanager::Client;
use serde_json::Value;
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct SecretsReader {
    client: Client,
}

impl SecretsReader {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    async fn fetch_secret_json(&self, secret_id: &str) -> Result<Value> {
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

    pub async fn get_secret_value(
        &self,
        secret_id: &str,
        key: &str,
        cache: &OnceCell<Value>,
    ) -> Result<String> {
        let json = cache
            .get_or_try_init(|| async { self.fetch_secret_json(secret_id).await })
            .await?;

        json.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .context(format!("Key '{}' not found in secret JSON", key))
    }
}
