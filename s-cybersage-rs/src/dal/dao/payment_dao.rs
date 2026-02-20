use anyhow::{Context, Result};
use aws_sdk_dynamodb::{types::AttributeValue, Client};
use std::time::{SystemTime, UNIX_EPOCH};

const SUBSCRIPTION_KEY: &str = "SUBSCRIPTION";

#[derive(Clone)]
pub struct PaymentDao {
    client: Client,
    table_name: String,
}

impl PaymentDao {
    pub fn new(client: Client, table_name: impl Into<String>) -> Self {
        Self {
            client,
            table_name: table_name.into(),
        }
    }

    pub async fn get_subscription_status(&self, guild_id: &str) -> Result<Option<String>> {
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
            .context("Failed to query GuildSubscriptions table")?;

        let item = match response.item {
            Some(item) => item,
            None => return Ok(None),
        };

        let status = item
            .get("status")
            .and_then(|v| v.as_s().ok())
            .map(|s| s.to_string());

        Ok(status)
    }

    pub async fn subscribe_guild(&self, guild_id: &str, duration_seconds: i64) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let expires_at = now + duration_seconds;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .item("guild_id", AttributeValue::S(guild_id.to_string()))
            .item(
                "subscription_key",
                AttributeValue::S(SUBSCRIPTION_KEY.to_string()),
            )
            .item("status", AttributeValue::S("active".to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()))
            .send()
            .await
            .context("Failed to subscribe guild")?;

        Ok(())
    }

    pub async fn cancel_subscription(&self, guild_id: &str) -> Result<()> {
        self.client
            .update_item()
            .table_name(&self.table_name)
            .key("guild_id", AttributeValue::S(guild_id.to_string()))
            .key(
                "subscription_key",
                AttributeValue::S(SUBSCRIPTION_KEY.to_string()),
            )
            .update_expression("SET #s = :inactive")
            .expression_attribute_names("#s", "status")
            .expression_attribute_values(":inactive", AttributeValue::S("inactive".to_string()))
            .send()
            .await
            .context("Failed to cancel subscription")?;

        Ok(())
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
