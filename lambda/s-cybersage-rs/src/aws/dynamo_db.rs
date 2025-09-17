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

    pub async fn get_role_id(&self, role_name: &str) -> Result<Option<String>> {
        let resp = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("roleName", AttributeValue::S(role_name.to_string()))
            .send()
            .await
            .context("Failed to get item from DynamoDB")?;

        if let Some(item) = resp.item {
            if let Some(role_id_attr) = item.get("roleId") {
                if let Ok(role_id) = role_id_attr.as_s() {
                    return Ok(Some(role_id.to_owned()));
                }
            }
        }

        Ok(None)
    }

    pub async fn scan_roles_by_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        let resp = self
            .client
            .scan()
            .table_name(&self.table_name)
            .send()
            .await
            .context("Failed to scan DynamoDB table")?;

        let prefix_lower = prefix.to_lowercase();

        let roles = resp
            .items
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                item.get("roleName")
                    .and_then(|attr| attr.as_s().ok())
                    .filter(|name| name.to_lowercase().starts_with(&prefix_lower))
                    .map(|name| name.to_string())
            })
            .take(25)
            .collect();

        Ok(roles)
    }
}
