use anyhow::{bail, Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy)]
pub enum RoleAction {
    Add,
    Remove,
}

#[derive(Debug, Deserialize)]
struct GuildMember {
    roles: Vec<String>,
}

pub struct RoleManager {
    client: Client,
    bot_token: String,
}

impl RoleManager {
    pub fn new(client: Client, bot_token: impl Into<String>) -> Self {
        Self {
            client,
            bot_token: bot_token.into(),
        }
    }

    pub async fn fetch_member_roles(&self, guild_id: &str, user_id: &str) -> Result<Vec<String>> {
        let url = format!(
            "https://discord.com/api/v10/guilds/{}/members/{}",
            guild_id, user_id
        );

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .await
            .context("Failed to send fetch_member_roles request")?;

        if resp.status() == StatusCode::NOT_FOUND {
            bail!("Member not found in guild");
        }

        if resp.status() == StatusCode::FORBIDDEN {
            bail!("Bot lacks permission to fetch member");
        }

        let resp = resp
            .error_for_status()
            .context("Discord returned error while fetching member")?;

        let member: GuildMember = resp
            .json()
            .await
            .context("Failed to deserialize GuildMember")?;

        Ok(member.roles)
    }

    pub async fn modify_user_role(
        &self,
        guild_id: &str,
        user_id: &str,
        role_id: &str,
        action: RoleAction,
    ) -> Result<()> {
        let url = format!(
            "https://discord.com/api/v10/guilds/{}/members/{}/roles/{}",
            guild_id, user_id, role_id
        );

        let request_builder = match action {
            RoleAction::Add => self.client.put(&url),
            RoleAction::Remove => self.client.delete(&url),
        };

        let resp = request_builder
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .await
            .context("Failed to send modify_user_role request")?;

        match resp.status() {
            status if status.is_success() => {
                info!(
                    "Successfully {:?} role {} for user {}",
                    action, role_id, user_id
                );
                Ok(())
            }

            StatusCode::FORBIDDEN => {
                error!(
                    "Permission error while {:?} role {} for user {}",
                    action, role_id, user_id
                );
                bail!("Bot lacks permission to modify role (check role hierarchy)")
            }

            StatusCode::NOT_FOUND => {
                error!(
                    "Role or user not found while {:?} role {} for user {}",
                    action, role_id, user_id
                );
                bail!("Role or user not found")
            }

            StatusCode::TOO_MANY_REQUESTS => {
                let body = resp.text().await.unwrap_or_default();
                warn!(
                    "Rate limited while {:?} role {} for user {}: {}",
                    action, role_id, user_id, body
                );
                bail!("Rate limited by Discord API")
            }

            other => {
                let body = resp.text().await.unwrap_or_default();
                error!(
                    "Failed to {:?} role {} for user {}: status {}, body: {}",
                    action, role_id, user_id, other, body
                );
                bail!("Discord API error: {}", other);
            }
        }
    }
}
