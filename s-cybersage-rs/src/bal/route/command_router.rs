use anyhow::Result;

use crate::{
    bal::discord::role_manager::{RoleAction, RoleManager},
    dal::{
        dao::guild::GuildDao,
        model::{
            interaction_request::{ApplicationCommandData, InteractionRequest},
            interaction_response::{ApplicationCommandOptionChoice, InteractionResponse},
        },
    },
};

pub struct CommandRouter {
    guild_dao: GuildDao,
    role_manager: RoleManager,
}

impl CommandRouter {
    pub fn new(guild_dao: GuildDao, role_manager: RoleManager) -> Self {
        Self {
            guild_dao,
            role_manager,
        }
    }

    pub async fn handle_autocomplete(
        &self,
        interaction: &InteractionRequest,
    ) -> Result<InteractionResponse> {
        let guild_id = interaction.guild_id.as_deref().unwrap_or("");

        let prefix = interaction
            .data
            .as_ref()
            .and_then(|cmd| cmd.options.first())
            .and_then(|sub| sub.options.first())
            .and_then(|opt| opt.value.as_ref())
            .and_then(|val| val.as_str())
            .unwrap_or("");

        let roles = self
            .guild_dao
            .query_roles_by_prefix(guild_id, prefix)
            .await
            .unwrap_or_default();

        let choices: Vec<ApplicationCommandOptionChoice> = roles
            .into_iter()
            .map(|(role_name, _)| ApplicationCommandOptionChoice {
                name: role_name.clone(),
                value: role_name,
            })
            .collect();

        Ok(InteractionResponse::autocomplete(choices))
    }

    pub async fn handle_command(
        &self,
        interaction: &InteractionRequest,
    ) -> Result<InteractionResponse> {
        let guild_id = interaction.guild_id.as_deref().unwrap_or("");

        let cmd_data: &ApplicationCommandData = match interaction.data.as_ref() {
            Some(d) => d,
            None => return Ok(InteractionResponse::ephemeral("Invalid command data.")),
        };

        match cmd_data.name.as_str() {
            "role" => {
                self.handle_role_command(guild_id, cmd_data, interaction)
                    .await
            }
            _ => Ok(InteractionResponse::ephemeral("Unknown command.")),
        }
    }

    async fn handle_role_command(
        &self,
        guild_id: &str,
        cmd_data: &ApplicationCommandData,
        interaction: &InteractionRequest,
    ) -> Result<InteractionResponse> {
        let subcommand = match cmd_data.options.first() {
            Some(s) => s,
            None => return Ok(InteractionResponse::ephemeral("Missing subcommand.")),
        };

        match subcommand.name.as_str() {
            "save" => {
                let role_id = subcommand
                    .options
                    .first()
                    .and_then(|opt| opt.value.as_ref())
                    .and_then(|val| val.as_str())
                    .unwrap_or("")
                    .to_string();

                if role_id.is_empty() {
                    return Ok(InteractionResponse::ephemeral("Role is required."));
                }

                let role_name = match cmd_data
                    .resolved
                    .as_ref()
                    .and_then(|r| r.roles.get(&role_id))
                    .map(|r| r.name.clone())
                {
                    Some(n) => n,
                    None => return Ok(InteractionResponse::ephemeral("Resolved role missing.")),
                };

                self.guild_dao
                    .save_role(guild_id, &role_id, &role_name)
                    .await?;

                Ok(InteractionResponse::ephemeral(
                    "Role registered successfully.",
                ))
            }

            "toggle" => {
                let role_name_input = subcommand
                    .options
                    .first()
                    .and_then(|opt| opt.value.as_ref())
                    .and_then(|val| val.as_str())
                    .unwrap_or("")
                    .to_string();

                let (role_name, role_id) = match self
                    .guild_dao
                    .get_role_by_name(guild_id, &role_name_input)
                    .await?
                {
                    Some(role) => role,
                    None => return Ok(InteractionResponse::ephemeral("Role not self-assignable.")),
                };

                let user_id = interaction
                    .member
                    .as_ref()
                    .map(|m| m.user.id.as_str())
                    .unwrap_or("");

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

            _ => Ok(InteractionResponse::ephemeral("Unknown subcommand.")),
        }
    }
}
