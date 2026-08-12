use anyhow::{Context, Result};
use aws_sdk_dynamodb::{types::AttributeValue, Client};

struct Keys;

impl Keys {
    pub fn pk(guild_id: &str) -> String {
        format!("GUILD#{}", guild_id)
    }

    pub fn role_sk(role_name: &str) -> String {
        format!("ROLE#{}", role_name.to_lowercase())
    }

    pub fn role_name_lookup_pk(guild_id: &str) -> String {
        Self::pk(guild_id)
    }

    pub fn role_name_lookup_sk(role_name: &str) -> String {
        format!("ROLE_NAME#{}", role_name.to_lowercase())
    }

    pub fn role_id_lookup_pk(guild_id: &str) -> String {
        Self::pk(guild_id)
    }

    pub fn role_id_lookup_sk(role_id: &str) -> String {
        format!("ROLE_ID#{}", role_id)
    }
}

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

    pub async fn query_roles_by_prefix(
        &self,
        guild_id: &str,
        prefix: &str,
    ) -> Result<Vec<(String, String)>> {
        if prefix.trim().is_empty() {
            return Ok(vec![]);
        }

        let normalized = prefix.to_lowercase();

        let response = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("LookupByRoleName")
            .key_condition_expression(
                "role_name_lookup_pk = :pk AND begins_with(role_name_lookup_sk, :prefix)",
            )
            .expression_attribute_values(
                ":pk",
                AttributeValue::S(Keys::role_name_lookup_pk(guild_id)),
            )
            .expression_attribute_values(
                ":prefix",
                AttributeValue::S(format!("ROLE_NAME#{}", normalized)),
            )
            .limit(25)
            .send()
            .await
            .context("Failed to query roles by prefix")?;

        Ok(response
            .items
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let name = item.get("role_name")?.as_s().ok()?.to_string();
                let id = item.get("role_id")?.as_s().ok()?.to_string();
                Some((name, id))
            })
            .collect())
    }

    pub async fn save_role(&self, guild_id: &str, role_id: &str, role_name: &str) -> Result<()> {
        let normalized = role_name.to_lowercase();

        let pk = Keys::pk(guild_id);

        self.client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(pk.clone()))
            .item("SK", AttributeValue::S(Keys::role_sk(&normalized)))
            .item("role_id", AttributeValue::S(role_id.to_string()))
            .item("role_name", AttributeValue::S(role_name.to_string()))
            .item(
                "role_name_lookup_pk",
                AttributeValue::S(Keys::role_name_lookup_pk(guild_id)),
            )
            .item(
                "role_name_lookup_sk",
                AttributeValue::S(Keys::role_name_lookup_sk(&normalized)),
            )
            .item(
                "role_id_lookup_pk",
                AttributeValue::S(Keys::role_id_lookup_pk(guild_id)),
            )
            .item(
                "role_id_lookup_sk",
                AttributeValue::S(Keys::role_id_lookup_sk(role_id)),
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
        let normalized = role_name.to_lowercase();

        let response = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("LookupByRoleName")
            .key_condition_expression("role_name_lookup_pk = :pk AND role_name_lookup_sk = :sk")
            .expression_attribute_values(
                ":pk",
                AttributeValue::S(Keys::role_name_lookup_pk(guild_id)),
            )
            .expression_attribute_values(
                ":sk",
                AttributeValue::S(Keys::role_name_lookup_sk(&normalized)),
            )
            .limit(1)
            .send()
            .await
            .context("Failed to query role by name")?;

        Ok(response
            .items
            .and_then(|mut items| items.pop())
            .and_then(|item| {
                let name = item.get("role_name")?.as_s().ok()?.to_string();
                let id = item.get("role_id")?.as_s().ok()?.to_string();
                Some((name, id))
            }))
    }
}
