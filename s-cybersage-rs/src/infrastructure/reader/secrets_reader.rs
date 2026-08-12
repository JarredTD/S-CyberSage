use anyhow::Result;
use aws_sdk_secretsmanager::Client;

/// Retrieves string values from JSON secrets in AWS Secrets Manager.
#[derive(Clone)]
pub struct SecretsReader {
    /// Secrets Manager client used to retrieve configured secret values.
    client: Client,
}

impl SecretsReader {
    /// Creates a secrets reader backed by the supplied AWS client.
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Retrieves a string field from a JSON-encoded secret.
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
