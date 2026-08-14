use anyhow::{bail, Context, Result};
use athenaeum::http::DiscordBotClient;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::application::ports::{MemberRoleGateway, RoleMembershipAction};

/// Bit position Discord assigns to the Manage Roles guild permission.
const MANAGE_ROLES_PERMISSION: u64 = 1 << 28;

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

/// Represents the Discord role fields needed to evaluate hierarchy and management restrictions.
#[derive(Debug, Deserialize)]
struct GuildRole {
    /// Stable Discord role identifier.
    id: String,
    /// Hierarchy position within the guild.
    position: i64,
    /// Whether Discord, rather than the guild, manages the role.
    managed: bool,
    /// Guild permissions granted by this role as a decimal bitfield string.
    permissions: String,
}

/// Implements Discord's REST API for guild member role operations.
pub struct RoleManager {
    /// Bot-authenticated client for Discord API requests.
    client: DiscordBotClient,
}

impl RoleManager {
    /// Creates a role manager authenticated with the supplied Discord bot token.
    ///
    /// # Arguments
    ///
    /// * `client` - Reusable HTTP client for Discord REST API calls.
    /// * `bot_token` - Discord bot token used to authenticate those calls.
    pub fn new(client: Client, bot_token: impl Into<String>) -> Self {
        Self { client: DiscordBotClient::new(client, bot_token) }
    }

    /// Creates a role manager that targets a custom Discord-compatible REST endpoint.
    ///
    /// # Arguments
    ///
    /// * `client` - Reusable HTTP client for REST API calls.
    /// * `bot_token` - Token used to authenticate REST API calls.
    /// * `api_base_url` - Base URL of the Discord-compatible REST API.
    #[cfg(test)]
    pub fn with_api_base_url(
        client: Client,
        bot_token: impl Into<String>,
        api_base_url: impl Into<String>,
    ) -> Self {
        Self { client: DiscordBotClient::with_api_base_url(client, bot_token, api_base_url) }
    }

    /// Determines whether the bot can manage a role according to Discord's role hierarchy.
    ///
    /// # Arguments
    ///
    /// * `guild_id` - Guild containing the bot and candidate role.
    /// * `role_id` - Candidate role to validate.
    ///
    /// # Errors
    ///
    /// Returns an error when Discord rejects the role or bot-membership lookups.
    pub async fn can_manage_role(&self, guild_id: &str, role_id: &str) -> Result<bool> {
        if role_id == guild_id {
            return Ok(false);
        }

        let roles: Vec<GuildRole> = self
            .client
            .get(&format!("guilds/{guild_id}/roles"))
            .send()
            .await
            .context("Failed to fetch Discord guild roles")?
            .error_for_status()
            .context("Discord rejected guild role lookup")?
            .json()
            .await
            .context("Failed to deserialize Discord guild roles")?;
        let Some(candidate_role) = roles.iter().find(|role| role.id == role_id) else {
            return Ok(false);
        };

        if candidate_role.managed {
            return Ok(false);
        }

        let bot_member: GuildMember = self
            .client
            .get(&format!("users/@me/guilds/{guild_id}/member"))
            .send()
            .await
            .context("Failed to fetch Discord bot guild membership")?
            .error_for_status()
            .context("Discord rejected bot guild membership lookup")?
            .json()
            .await
            .context("Failed to deserialize Discord bot guild membership")?;
        let bot_roles: Vec<&GuildRole> =
            roles.iter().filter(|role| bot_member.roles.iter().any(|id| id == &role.id)).collect();
        let highest_bot_position =
            bot_roles.iter().map(|role| role.position).max().unwrap_or_default();
        let bot_permissions = bot_roles.iter().fold(0_u64, |permissions, role| {
            permissions | role.permissions.parse::<u64>().unwrap_or_default()
        });

        Ok(bot_permissions & MANAGE_ROLES_PERMISSION != 0
            && candidate_role.position < highest_bot_position)
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
        let path = format!("guilds/{guild_id}/members/{user_id}");

        let resp = self
            .client
            .get(&path)
            .send()
            .await
            .context("Failed to send fetch_member_roles request")?;

        match resp.status() {
            StatusCode::NOT_FOUND => bail!("Member not found in guild"),
            StatusCode::FORBIDDEN => bail!("Bot lacks permission to fetch member"),
            _ => {}
        }

        let resp =
            resp.error_for_status().context("Discord returned error while fetching member")?;

        let member: GuildMember = resp.json().await.context("Failed to deserialize GuildMember")?;

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
        let path = format!("guilds/{guild_id}/members/{user_id}/roles/{role_id}");

        let request = match action {
            RoleAction::Add => self.client.put(&path),
            RoleAction::Remove => self.client.delete(&path),
        };

        let resp = request.send().await.context("Failed to send modify_user_role request")?;

        match resp.status() {
            status if status.is_success() => {
                info!(guild_id, user_id, role_id, ?action, "Role modification succeeded");
                Ok(())
            }

            StatusCode::FORBIDDEN => {
                error!(guild_id, user_id, role_id, ?action, "Permission error");
                bail!("Bot lacks permission to modify role")
            }

            StatusCode::NOT_FOUND => {
                error!(guild_id, user_id, role_id, ?action, "Role or user not found");
                bail!("Role or user not found")
            }

            StatusCode::TOO_MANY_REQUESTS => {
                let body = resp.text().await.unwrap_or_default();

                warn!(guild_id, user_id, role_id, ?action, body, "Rate limited by Discord");

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
    async fn can_manage_role(&self, guild_id: &str, role_id: &str) -> Result<bool> {
        RoleManager::can_manage_role(self, guild_id, role_id).await
    }

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

    /// Confirms that roles above the bot's highest role are rejected before registration.
    #[tokio::test]
    async fn rejects_role_above_bot_hierarchy() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/guild/roles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "bot-role", "position": 10, "managed": false, "permissions": "268435456" },
                { "id": "candidate", "position": 11, "managed": false, "permissions": "0" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/users/@me/guilds/guild/member"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "roles": ["bot-role"]
            })))
            .mount(&server)
            .await;
        let manager = RoleManager::with_api_base_url(reqwest::Client::new(), "token", server.uri());

        assert!(!manager
            .can_manage_role("guild", "candidate")
            .await
            .expect("mocked Discord hierarchy lookup should succeed"));
    }

    /// Confirms that a bot without Manage Roles cannot register an otherwise lower role.
    #[tokio::test]
    async fn rejects_role_when_bot_lacks_manage_roles_permission() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/guild/roles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "bot-role", "position": 10, "managed": false, "permissions": "0" },
                { "id": "candidate", "position": 5, "managed": false, "permissions": "0" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/users/@me/guilds/guild/member"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "roles": ["bot-role"]
            })))
            .mount(&server)
            .await;
        let manager = RoleManager::with_api_base_url(reqwest::Client::new(), "token", server.uri());

        assert!(!manager
            .can_manage_role("guild", "candidate")
            .await
            .expect("mocked Discord permission lookup should succeed"));
    }
}
