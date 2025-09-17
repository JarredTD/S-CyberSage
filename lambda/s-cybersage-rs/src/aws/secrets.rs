use anyhow::{Context, Result};
use aws_sdk_secretsmanager::Client;
use serde_json::Value;

pub struct SecretsManager {
    client: Client,
}

impl SecretsManager {
    pub async fn new() -> Result<Self> {
        let config = aws_config::load_from_env().await;
        let client = Client::new(&config);
        Ok(Self { client })
    }

    pub async fn get_secret(&self, secret_id: &str, key: &str) -> Result<String> {
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

        let json_val: Value =
            serde_json::from_str(secret_str).context("Failed to parse secret string as JSON")?;

        json_val
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .context(format!("Key '{}' not found in secret JSON", key))
    }
}
