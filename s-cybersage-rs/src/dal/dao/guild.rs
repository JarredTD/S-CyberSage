use anyhow::{Context, Result};
use aws_sdk_dynamodb::{types::AttributeValue, Client};

pub struct GuildDao {
    client: Client,
    table_name: String,
}

impl GuildDao {
    pub fn new(client: Client, table_name: impl Into<String>) -> Self {
        Self {
            client,
            table_name: table_name.into(),
        }
    }

    pub async fn get_role_by_id(
        &self,
        guild_id: &str,
        role_id: &str,
    ) -> Result<Option<(String, String)>> {
        let response = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("guild_id", AttributeValue::S(guild_id.to_string()))
            .key(
                "mapping_key",
                AttributeValue::S(format!("ROLE#{}", role_id)),
            )
            .send()
            .await
            .context("Failed to get role by ID")?;

        if let Some(item) = response.item {
            let role_name = item
                .get("role_name")
                .and_then(|v| v.as_s().ok())
                .map(|s| s.to_string());

            let role_id = item
                .get("role_id")
                .and_then(|v| v.as_s().ok())
                .map(|s| s.to_string());

            if let (Some(name), Some(id)) = (role_name, role_id) {
                return Ok(Some((name, id)));
            }
        }

        Ok(None)
    }

    pub async fn query_roles_by_prefix(
        &self,
        guild_id: &str,
        prefix: &str,
    ) -> Result<Vec<(String, String)>> {
        if prefix.trim().is_empty() {
            return Ok(vec![]);
        }

        let normalized_prefix = prefix.to_lowercase();

        let response = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("GuildRoleNameIndex")
            .key_condition_expression(
                "guild_id = :guild_id AND begins_with(role_name_normalized, :prefix)",
            )
            .expression_attribute_values(":guild_id", AttributeValue::S(guild_id.to_string()))
            .expression_attribute_values(":prefix", AttributeValue::S(normalized_prefix))
            .limit(25)
            .send()
            .await
            .context("Failed to query roles by prefix")?;

        let roles = response
            .items
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let role_name = item.get("role_name")?.as_s().ok()?.to_string();
                let role_id = item.get("role_id")?.as_s().ok()?.to_string();
                Some((role_name, role_id))
            })
            .collect();

        Ok(roles)
    }

    pub async fn save_role(&self, guild_id: &str, role_id: &str, role_name: &str) -> Result<()> {
        let normalized_name = role_name.to_lowercase();

        self.client
            .put_item()
            .table_name(&self.table_name)
            .item("guild_id", AttributeValue::S(guild_id.to_string()))
            .item(
                "mapping_key",
                AttributeValue::S(format!("ROLE#{}", role_id)),
            )
            .item("role_id", AttributeValue::S(role_id.to_string()))
            .item("role_name", AttributeValue::S(role_name.to_string()))
            .item("role_name_normalized", AttributeValue::S(normalized_name))
            .send()
            .await
            .context("Failed to save role")?;

        Ok(())
    }

    pub async fn get_role_by_name(
        &self,
        guild_id: &str,
        role_name: &str,
    ) -> Result<Option<(String, String)>> {
        let normalized_name = role_name.to_lowercase();

        let response = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("GuildRoleNameIndex")
            .key_condition_expression("guild_id = :guild_id AND role_name_normalized = :role_name")
            .expression_attribute_values(":guild_id", AttributeValue::S(guild_id.to_string()))
            .expression_attribute_values(":role_name", AttributeValue::S(normalized_name))
            .limit(1)
            .send()
            .await
            .context("Failed to query role by name")?;

        if let Some(mut items) = response.items {
            if let Some(item) = items.pop() {
                let role_name = item
                    .get("role_name")
                    .and_then(|v| v.as_s().ok())
                    .map(|s| s.to_string());

                let role_id = item
                    .get("role_id")
                    .and_then(|v| v.as_s().ok())
                    .map(|s| s.to_string());

                if let (Some(name), Some(id)) = (role_name, role_id) {
                    return Ok(Some((name, id)));
                }
            }
        }

        Ok(None)
    }
}
