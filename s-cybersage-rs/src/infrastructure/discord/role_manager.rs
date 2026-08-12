use anyhow::{bail, Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::application::ports::{MemberRoleGateway, RoleMembershipAction};

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

/// Implements Discord's REST API for guild member role operations.
pub struct RoleManager {
    /// Reusable HTTP client for Discord API requests.
    client: Client,
    /// Preformatted bot authorization header.
    auth_header: String,
    /// Base URL for Discord REST API requests.
    api_base_url: String,
}

impl RoleManager {
    /// Creates a role manager authenticated with the supplied Discord bot token.
    ///
    /// # Arguments
    ///
    /// * `client` - Reusable HTTP client for Discord REST API calls.
    /// * `bot_token` - Discord bot token used to authenticate those calls.
    pub fn new(client: Client, bot_token: impl Into<String>) -> Self {
        let token = bot_token.into();

        Self {
            client,
            auth_header: format!("Bot {}", token),
            api_base_url: DISCORD_API_BASE.to_string(),
        }
    }

    /// Creates a role manager that targets a custom Discord-compatible REST endpoint.
    ///
    /// # Arguments
    ///
    /// * `client` - Reusable HTTP client for REST API calls.
    /// * `bot_token` - Token used to authenticate REST API calls.
    /// * `api_base_url` - Base URL of the Discord-compatible REST API.
    pub fn with_api_base_url(
        client: Client,
        bot_token: impl Into<String>,
        api_base_url: impl Into<String>,
    ) -> Self {
        let mut manager = Self::new(client, bot_token);
        manager.api_base_url = api_base_url.into().trim_end_matches('/').to_string();
        manager
    }

    /// Retrieves the role IDs assigned to a guild member.
    ///
    /// # Arguments
    ///
    /// * `guild_id` - Discord guild containing the member.
    /// * `user_id` - Discord user whose membership is queried.
    ///
    /// # Errors
    ///
    /// Returns an error when the member does not exist, the bot lacks permission, or Discord's
    /// REST API request or response fails.
    pub async fn fetch_member_roles(&self, guild_id: &str, user_id: &str) -> Result<Vec<String>> {
        let url = format!(
            "{}/guilds/{}/members/{}",
            self.api_base_url, guild_id, user_id
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
    ///
    /// # Arguments
    ///
    /// * `guild_id` - Discord guild containing the member and role.
    /// * `user_id` - Discord user whose role membership changes.
    /// * `role_id` - Discord role to add or remove.
    /// * `action` - Whether the role is added or removed.
    ///
    /// # Errors
    ///
    /// Returns an error when Discord rejects the change, including missing permissions,
    /// missing resources, or rate limiting.
    pub async fn modify_user_role(
        &self,
        guild_id: &str,
        user_id: &str,
        role_id: &str,
        action: RoleAction,
    ) -> Result<()> {
        let url = format!(
            "{}/guilds/{}/members/{}/roles/{}",
            self.api_base_url, guild_id, user_id, role_id
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

impl MemberRoleGateway for RoleManager {
    async fn fetch_member_roles(&self, guild_id: &str, user_id: &str) -> Result<Vec<String>> {
        RoleManager::fetch_member_roles(self, guild_id, user_id).await
    }

    async fn modify_user_role(
        &self,
        guild_id: &str,
        user_id: &str,
        role_id: &str,
        action: RoleMembershipAction,
    ) -> Result<()> {
        let action = match action {
            RoleMembershipAction::Add => RoleAction::Add,
            RoleMembershipAction::Remove => RoleAction::Remove,
        };

        RoleManager::modify_user_role(self, guild_id, user_id, role_id, action).await
    }
}

/// Tests Discord REST adapter requests against a local HTTP server.
#[cfg(test)]
mod tests {
    use super::{RoleAction, RoleManager};
    use wiremock::{
        matchers::{header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    /// Confirms that member lookup sends bot authentication and parses role IDs.
    #[tokio::test]
    async fn fetches_member_roles() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/guild/members/user"))
            .and(header("authorization", "Bot token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "roles": ["role-a", "role-b"]
            })))
            .mount(&server)
            .await;
        let manager = RoleManager::with_api_base_url(reqwest::Client::new(), "token", server.uri());

        let roles = manager
            .fetch_member_roles("guild", "user")
            .await
            .expect("mocked Discord lookup should succeed");

        assert_eq!(roles, ["role-a", "role-b"]);
    }

    /// Confirms that a role addition uses Discord's expected PUT endpoint.
    #[tokio::test]
    async fn adds_member_role() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/guilds/guild/members/user/roles/role"))
            .and(header("authorization", "Bot token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let manager = RoleManager::with_api_base_url(reqwest::Client::new(), "token", server.uri());

        manager
            .modify_user_role("guild", "user", "role", RoleAction::Add)
            .await
            .expect("mocked Discord role addition should succeed");
    }
}
