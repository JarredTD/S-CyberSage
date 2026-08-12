use anyhow::{anyhow, Result};

use crate::{
    application::discord::role_manager::{RoleAction, RoleManager},
    infrastructure::dao::guild::GuildDao,
    transport::discord::{
        interaction_request::{ApplicationCommandData, CommandOption, InteractionRequest},
        interaction_response::{ApplicationCommandOptionChoice, InteractionResponse},
    },
};

/// Bit position Discord assigns to the Administrator guild permission.
const ADMINISTRATOR_PERMISSION: u64 = 1 << 3;

/// Routes Discord role commands to persistence and Discord API operations.
pub struct CommandRouter {
    /// Stores self-assignable roles for each guild.
    guild_dao: GuildDao,
    /// Applies role changes through Discord's REST API.
    role_manager: RoleManager,
}

impl CommandRouter {
    /// Creates a command router from its persistence and Discord dependencies.
    pub fn new(guild_dao: GuildDao, role_manager: RoleManager) -> Self {
        Self {
            guild_dao,
            role_manager,
        }
    }

    /// Returns matching self-assignable roles for a Discord autocomplete interaction.
    pub async fn handle_autocomplete(
        &self,
        interaction: &InteractionRequest,
    ) -> Result<InteractionResponse> {
        let guild_id = require_guild_id(interaction)?;

        let prefix = interaction
            .data
            .as_ref()
            .and_then(extract_first_option_value)
            .unwrap_or("");

        let roles = self
            .guild_dao
            .query_roles_by_prefix(guild_id, prefix)
            .await?;

        let choices = roles
            .into_iter()
            .map(|(name, _)| ApplicationCommandOptionChoice {
                name: name.clone(),
                value: name,
            })
            .collect();

        Ok(InteractionResponse::autocomplete(choices))
    }

    /// Handles a Discord application command interaction.
    #[tracing::instrument(skip(self, interaction))]
    pub async fn handle_command(
        &self,
        interaction: &InteractionRequest,
    ) -> Result<InteractionResponse> {
        let guild_id = require_guild_id(interaction)?;

        let cmd_data = interaction
            .data
            .as_ref()
            .ok_or_else(|| anyhow!("Missing command data"))?;

        match cmd_data.name.as_str() {
            "role" => {
                self.handle_role_command(guild_id, cmd_data, interaction)
                    .await
            }
            _ => Ok(InteractionResponse::ephemeral("Unknown command.")),
        }
    }

    /// Dispatches the requested role subcommand.
    async fn handle_role_command(
        &self,
        guild_id: &str,
        cmd_data: &ApplicationCommandData,
        interaction: &InteractionRequest,
    ) -> Result<InteractionResponse> {
        let subcommand = cmd_data
            .options
            .first()
            .ok_or_else(|| anyhow!("Missing subcommand"))?;

        match subcommand.name.as_str() {
            "save" => {
                self.handle_save(guild_id, subcommand, cmd_data, interaction)
                    .await
            }
            "toggle" => self.handle_toggle(guild_id, subcommand, interaction).await,
            _ => Ok(InteractionResponse::ephemeral("Unknown subcommand.")),
        }
    }

    /// Saves an administrator-selected role as self-assignable.
    async fn handle_save(
        &self,
        guild_id: &str,
        subcommand: &CommandOption,
        cmd_data: &ApplicationCommandData,
        interaction: &InteractionRequest,
    ) -> Result<InteractionResponse> {
        if !has_administrator_permission(interaction) {
            return Ok(InteractionResponse::ephemeral(
                "Administrator permission is required to register roles.",
            ));
        }

        let role_id = subcommand
            .options
            .first()
            .and_then(|opt: &CommandOption| opt.value.as_ref())
            .and_then(|val| val.as_str())
            .ok_or_else(|| anyhow!("Role is required"))?;

        let role_name = cmd_data
            .resolved
            .as_ref()
            .and_then(|r| r.roles.get(role_id))
            .map(|r| r.name.clone())
            .ok_or_else(|| anyhow!("Resolved role missing"))?;

        self.guild_dao
            .save_role(guild_id, role_id, &role_name)
            .await?;

        Ok(InteractionResponse::ephemeral(
            "Role registered successfully.",
        ))
    }

    /// Adds or removes the selected self-assignable role for the invoking member.
    async fn handle_toggle(
        &self,
        guild_id: &str,
        subcommand: &CommandOption,
        interaction: &InteractionRequest,
    ) -> Result<InteractionResponse> {
        let role_name_input = subcommand
            .options
            .first()
            .and_then(|opt: &CommandOption| opt.value.as_ref())
            .and_then(|val| val.as_str())
            .ok_or_else(|| anyhow!("Role name is required"))?;

        let (role_name, role_id) = self
            .guild_dao
            .get_role_by_name(guild_id, role_name_input)
            .await?
            .ok_or_else(|| anyhow!("Role not self-assignable"))?;

        let user_id = require_user_id(interaction)?;

        let member_roles = self
            .role_manager
            .fetch_member_roles(guild_id, user_id)
            .await?;

        let has_role = member_roles.iter().any(|r| r == &role_id);

        let action = if has_role {
            RoleAction::Remove
        } else {
            RoleAction::Add
        };

        self.role_manager
            .modify_user_role(guild_id, user_id, &role_id, action)
            .await?;

        let message = if has_role {
            format!("Removed '{}'.", role_name)
        } else {
            format!("Added '{}'.", role_name)
        };

        Ok(InteractionResponse::ephemeral(message))
    }
}

/// Returns the guild ID required by a guild-scoped interaction.
fn require_guild_id(interaction: &InteractionRequest) -> Result<&str> {
    interaction
        .guild_id
        .as_deref()
        .ok_or_else(|| anyhow!("Missing guild_id"))
}

/// Returns the invoking user's ID required by a guild interaction.
fn require_user_id(interaction: &InteractionRequest) -> Result<&str> {
    interaction
        .member
        .as_ref()
        .map(|m| m.user.id.as_str())
        .ok_or_else(|| anyhow!("Missing user_id"))
}

/// Returns whether Discord supplied the Administrator permission for the invoking member.
fn has_administrator_permission(interaction: &InteractionRequest) -> bool {
    interaction
        .member
        .as_ref()
        .and_then(|member| member.permissions.as_deref())
        .and_then(|permissions| permissions.parse::<u64>().ok())
        .is_some_and(|permissions| permissions & ADMINISTRATOR_PERMISSION != 0)
}

/// Extracts the focused option value from an autocomplete command payload.
fn extract_first_option_value(cmd: &ApplicationCommandData) -> Option<&str> {
    cmd.options
        .first()
        .and_then(|sub| sub.options.first())
        .and_then(|opt: &CommandOption| opt.value.as_ref())
        .and_then(|val| val.as_str())
}

/// Tests authorization decisions based on Discord's signed member permissions.
#[cfg(test)]
mod tests {
    use super::has_administrator_permission;
    use crate::transport::discord::interaction_request::{
        InteractionRequest, InteractionType, Member, User,
    };

    /// Confirms that the Administrator permission grants access to role registration.
    #[test]
    fn permits_administrator() {
        assert!(has_administrator_permission(&interaction_with_permissions(
            Some("8")
        )));
    }

    /// Confirms that absent, malformed, and unrelated permissions are denied.
    #[test]
    fn denies_non_administrators() {
        assert!(!has_administrator_permission(
            &interaction_with_permissions(None)
        ));
        assert!(!has_administrator_permission(
            &interaction_with_permissions(Some("invalid"))
        ));
        assert!(!has_administrator_permission(
            &interaction_with_permissions(Some("32"))
        ));
    }

    /// Builds the minimal guild interaction required for permission checks.
    fn interaction_with_permissions(permissions: Option<&str>) -> InteractionRequest {
        InteractionRequest {
            id: None,
            application_id: None,
            token: None,
            interaction_type: InteractionType::ApplicationCommand,
            data: None,
            guild_id: Some("guild".to_string()),
            member: Some(Member {
                permissions: permissions.map(str::to_string),
                user: User {
                    id: "user".to_string(),
                },
            }),
        }
    }
}
