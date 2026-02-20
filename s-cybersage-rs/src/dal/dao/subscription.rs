use anyhow::{Context, Result};
use aws_sdk_dynamodb::{types::AttributeValue, Client};

const SUBSCRIPTION_KEY: &str = "SUBSCRIPTION";

#[derive(Clone)]
pub struct SubscriptionReader {
    client: Client,
    table_name: String,
}

impl SubscriptionReader {
    pub fn new(client: Client, table_name: impl Into<String>) -> Self {
        Self {
            client,
            table_name: table_name.into(),
        }
    }

    pub async fn is_active(&self, guild_id: &str) -> Result<bool> {
        let response = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("guild_id", AttributeValue::S(guild_id.to_string()))
            .key(
                "subscription_key",
                AttributeValue::S(SUBSCRIPTION_KEY.to_string()),
            )
            .send()
            .await
            .context("Failed to query subscription")?;

        let item = match response.item {
            Some(item) => item,
            None => return Ok(false),
        };

        let status = item
            .get("status")
            .and_then(|v| v.as_s().ok())
            .map(|s| s.as_str())
            .unwrap_or("inactive");

        if status != "active" {
            return Ok(false);
        }

        let expires_at = item
            .get("expires_at")
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<i64>().ok())
            .unwrap_or(0);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        Ok(now <= expires_at)
    }
}
