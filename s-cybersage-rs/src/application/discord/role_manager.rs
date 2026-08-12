use anyhow::{bail, Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tracing::{error, info, warn};

/// Base URL for Discord's version 10 REST API.
const DISCORD_API_BASE: &str = "https://discord.com/api/v10";

/// Describes the role membership change to apply to a guild member.
#[derive(Debug, Clone, Copy)]
pub enum RoleAction {
    /// Adds the role to the member.
    Add,
    /// Removes the role from the member.
    Remove,
}

/// Represents the subset of a Discord guild member returned by the REST API.
#[derive(Debug, Deserialize)]
struct GuildMember {
    /// Role IDs currently assigned to the member.
    roles: Vec<String>,
}

/// Calls Discord's REST API to inspect and update guild member roles.
pub struct RoleManager {
    /// Reusable HTTP client for Discord API requests.
    client: Client,
    /// Preformatted bot authorization header.
    auth_header: String,
}

impl RoleManager {
    /// Creates a role manager authenticated with the supplied Discord bot token.
    pub fn new(client: Client, bot_token: impl Into<String>) -> Self {
        let token = bot_token.into();

        Self {
            client,
            auth_header: format!("Bot {}", token),
        }
    }

    /// Retrieves the role IDs assigned to a guild member.
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

    /// Adds or removes a role from a guild member.
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
