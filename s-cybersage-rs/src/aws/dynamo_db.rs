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
        let role_name_lower = role_name.to_lowercase();

        let resp = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("PK = :pk AND SK = :sk")
            .expression_attribute_values(":pk", AttributeValue::S("ROLE".into()))
            .expression_attribute_values(":sk", AttributeValue::S(role_name_lower))
            .limit(1)
            .send()
            .await
            .context("Failed to query DynamoDB for role")?;

        Ok(resp
            .items
            .unwrap_or_default()
            .into_iter()
            .next()
            .and_then(|item| item.get("roleId").cloned())
            .and_then(|attr| attr.as_s().ok().map(|s| s.to_string())))
    }

    pub async fn query_roles_by_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        if prefix.is_empty() {
            return Ok(vec![]);
        }

        let prefix_lower = prefix.to_lowercase();

        let resp = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
            .expression_attribute_values(":pk", AttributeValue::S("ROLE".into()))
            .expression_attribute_values(":prefix", AttributeValue::S(prefix_lower))
            .limit(25)
            .send()
            .await
            .context("Failed to query DynamoDB for role prefix")?;

        let roles = resp
            .items
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                item.get("SK")
                    .and_then(|attr| attr.as_s().ok())
                    .map(|s| s.to_string())
            })
            .collect();

        Ok(roles)
    }
}
