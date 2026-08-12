use anyhow::{bail, Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tracing::{error, info, warn};

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";

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
    auth_header: String,
}

impl RoleManager {
    pub fn new(client: Client, bot_token: impl Into<String>) -> Self {
        let token = bot_token.into();

        Self {
            client,
            auth_header: format!("Bot {}", token),
        }
    }

    pub async fn fetch_member_roles(&self, guild_id: &str, user_id: &str) -> Result<Vec<String>> {
        let url = format!(
            "{}/guilds/{}/members/{}",
            DISCORD_API_BASE, guild_id, user_id
        );

        let resp = self
            .client
            .get(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await
            .context("Failed to send fetch_member_roles request")?;

        match resp.status() {
            StatusCode::NOT_FOUND => bail!("Member not found in guild"),
            StatusCode::FORBIDDEN => bail!("Bot lacks permission to fetch member"),
            _ => {}
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
            "{}/guilds/{}/members/{}/roles/{}",
            DISCORD_API_BASE, guild_id, user_id, role_id
        );

        let request = match action {
            RoleAction::Add => self.client.put(&url),
            RoleAction::Remove => self.client.delete(&url),
        };

        let resp = request
            .header("Authorization", &self.auth_header)
            .send()
            .await
            .context("Failed to send modify_user_role request")?;

        match resp.status() {
            status if status.is_success() => {
                info!(
                    guild_id,
                    user_id,
                    role_id,
                    ?action,
                    "Role modification succeeded"
                );
                Ok(())
            }

            StatusCode::FORBIDDEN => {
                error!(guild_id, user_id, role_id, ?action, "Permission error");
                bail!("Bot lacks permission to modify role")
            }

            StatusCode::NOT_FOUND => {
                error!(
                    guild_id,
                    user_id,
                    role_id,
                    ?action,
                    "Role or user not found"
                );
                bail!("Role or user not found")
            }

            StatusCode::TOO_MANY_REQUESTS => {
                let body = resp.text().await.unwrap_or_default();

                warn!(
                    guild_id,
                    user_id,
                    role_id,
                    ?action,
                    body,
                    "Rate limited by Discord"
                );

                bail!("Rate limited by Discord API")
            }

            other => {
                let body = resp.text().await.unwrap_or_default();

                error!(
                    guild_id,
                    user_id,
                    role_id,
                    ?action,
                    status = %other,
                    body,
                    "Discord API error"
                );

                bail!("Discord API error: {}", other);
            }
        }
    }
}
