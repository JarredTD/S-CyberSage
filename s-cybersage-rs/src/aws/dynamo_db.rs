use anyhow::{Context, Result};
use aws_sdk_dynamodb::{types::AttributeValue, Client};

pub struct RoleDb {
    client: Client,
    table_name: String,
}

impl RoleDb {
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
        let resp = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("guild_id", AttributeValue::S(guild_id.to_string()))
            .key("entity_key", AttributeValue::S(format!("ROLE#{}", role_id)))
            .send()
            .await
            .context("Failed to get role by ID")?;

        if let Some(item) = resp.item {
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
        if prefix.is_empty() {
            return Ok(vec![]);
        }

        let prefix_lower = prefix.to_lowercase();

        let resp = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("role-name-index")
            .key_condition_expression(
                "guild_id = :guild_id AND begins_with(role_name_key, :prefix)",
            )
            .expression_attribute_values(":guild_id", AttributeValue::S(guild_id.to_string()))
            .expression_attribute_values(
                ":prefix",
                AttributeValue::S(format!("ROLE#{}", prefix_lower)),
            )
            .limit(25)
            .send()
            .await
            .context("Failed to query role-name-index")?;

        let roles = resp
            .items
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let role_name = item
                    .get("role_name")
                    .and_then(|v| v.as_s().ok())?
                    .to_string();

                let role_id = item.get("role_id").and_then(|v| v.as_s().ok())?.to_string();

                Some((role_name, role_id))
            })
            .collect();

        Ok(roles)
    }

    pub async fn save_role(&self, guild_id: &str, role_id: &str, role_name: &str) -> Result<()> {
        let role_name_lower = role_name.to_lowercase();

        self.client
            .put_item()
            .table_name(&self.table_name)
            .item("guild_id", AttributeValue::S(guild_id.to_string()))
            .item("entity_key", AttributeValue::S(format!("ROLE#{}", role_id)))
            .item("role_id", AttributeValue::S(role_id.to_string()))
            .item("role_name", AttributeValue::S(role_name.to_string()))
            .item(
                "role_name_lower",
                AttributeValue::S(role_name_lower.clone()),
            )
            .item(
                "role_name_key",
                AttributeValue::S(format!("ROLE#{}", role_name_lower)),
            )
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
        let role_name_lower = role_name.to_lowercase();

        let resp = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("role-name-index")
            .key_condition_expression("guild_id = :guild_id AND role_name_key = :role_key")
            .expression_attribute_values(":guild_id", AttributeValue::S(guild_id.to_string()))
            .expression_attribute_values(
                ":role_key",
                AttributeValue::S(format!("ROLE#{}", role_name_lower)),
            )
            .limit(1)
            .send()
            .await
            .context("Failed to query role by name")?;

        if let Some(item) = resp.items.and_then(|mut v| v.pop()) {
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
}
