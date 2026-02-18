use anyhow::{Context, Result};
use aws_sdk_secretsmanager::Client;
use serde_json::Value;
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct SecretsManager {
    client: Client,
}

impl SecretsManager {
    pub fn new_with_client(client: Client) -> Self {
        Self { client }
    }

    async fn fetch_secret_value(&self, secret_id: &str) -> Result<Value> {
        let resp = self
            .client
            .get_secret_value()
            .secret_id(secret_id)
            .send()
            .await
            .context("Failed to retrieve secret value from Secrets Manager")?;

        let secret_str = resp
            .secret_string()
            .context("Secret value is missing or not a string")?;

        serde_json::from_str(secret_str)
            .context("Failed to parse secret string as JSON")
    }

    pub async fn get_secret_cached(
        &self,
        secret_id: &str,
        key: &str,
        cache: &OnceCell<Value>,
    ) -> Result<String> {
        let json_val = cache
            .get_or_try_init(|| async { self.fetch_secret_value(secret_id).await })
            .await?;

        json_val
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .context(format!("Key '{}' not found in secret JSON", key))
    }
}
