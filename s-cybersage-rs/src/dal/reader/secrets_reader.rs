use anyhow::Result;
use aws_sdk_secretsmanager::Client;

#[derive(Clone)]
pub struct SecretsReader {
    client: Client,
}

impl SecretsReader {
    pub fn new(client: Client) -> Self {
        Self { client }
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
}
