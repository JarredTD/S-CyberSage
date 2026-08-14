use anyhow::{anyhow, Result};
use athenaeum::discord::DiscordPermissions;
use athenaeum::interaction::{ApplicationCommandOptionChoice, Interaction, InteractionResponse};

use crate::application::ports::{
    GuildRoleRepository, MemberRoleGateway, RoleMembershipAction, RoleRegistration,
};

use super::command_data::{ApplicationCommandData, CommandOption};

/// CyberSage command interaction carrying the bot's command schema.
type InteractionRequest = Interaction<ApplicationCommandData>;

/// Routes Discord role commands to persistence and Discord API operations.
pub struct CommandRouter<R, G> {
    /// Stores self-assignable roles for each guild.
    guild_repository: R,
    /// Applies role changes through Discord's REST API.
    member_role_gateway: G,
}

impl<R, G> CommandRouter<R, G> {
    /// Creates a command router from its persistence and Discord dependencies.
    ///
    /// # Arguments
    ///
    /// * `guild_repository` - Persistence adapter for registered self-assignable roles.
    /// * `member_role_gateway` - Discord adapter that changes member role assignments.
    pub fn new(guild_repository: R, member_role_gateway: G) -> Self {
        Self {
            guild_repository,
            member_role_gateway,
        }
    }
}

impl<R, G> CommandRouter<R, G>
where
    R: GuildRoleRepository,
    G: MemberRoleGateway,
{
    /// Returns matching self-assignable roles for a Discord autocomplete interaction.
    ///
    /// # Arguments
    ///
    /// * `interaction` - Verified autocomplete interaction with a guild and focused option.
    ///
    /// # Errors
    ///
    /// Returns an error when the interaction lacks a guild ID or DynamoDB cannot retrieve roles.
    pub async fn handle_autocomplete(
        &self,
        interaction: &Interaction<ApplicationCommandData>,
    ) -> Result<InteractionResponse> {
        let guild_id = require_guild_id(interaction)?;

        let prefix = interaction
            .data
            .as_ref()
            .and_then(extract_first_option_value)
            .unwrap_or("");

        let roles = self
            .guild_repository
            .query_roles_by_prefix(guild_id, prefix)
            .await?;

        let choices = roles
            .into_iter()
            .map(|role| ApplicationCommandOptionChoice {
                name: role.name.clone(),
                value: role.name,
            })
            .collect();

        Ok(InteractionResponse::autocomplete(choices))
    }

    /// Handles a Discord application command interaction.
    ///
    /// # Arguments
    ///
    /// * `interaction` - Verified command interaction to dispatch.
    ///
    /// # Errors
    ///
    /// Returns an error when required command fields are missing or a persistence or Discord API
    /// operation fails.
    #[tracing::instrument(skip(self, interaction))]
    pub async fn handle_command(
        &self,
        interaction: &Interaction<ApplicationCommandData>,
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

        if !self
            .member_role_gateway
            .can_manage_role(guild_id, role_id)
            .await?
        {
            return Ok(InteractionResponse::ephemeral(
                "I need Manage Roles permission and a bot role above this role before I can register it.",
            ));
        }

        self.guild_repository
            .save_role(
                guild_id,
                &RoleRegistration {
                    id: role_id.to_string(),
                    name: role_name,
                },
            )
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

        let role = self
            .guild_repository
            .get_role_by_name(guild_id, role_name_input)
            .await?
            .ok_or_else(|| anyhow!("Role not self-assignable"))?;

        let user_id = require_user_id(interaction)?;

        let member_roles = self
            .member_role_gateway
            .fetch_member_roles(guild_id, user_id)
            .await?;

        let has_role = member_roles.iter().any(|role_id| role_id == &role.id);

        let action = if has_role {
            RoleMembershipAction::Remove
        } else {
            RoleMembershipAction::Add
        };

        self.member_role_gateway
            .modify_user_role(guild_id, user_id, &role.id, action)
            .await?;

        let message = if has_role {
            format!("Removed '{}'.", role.name)
        } else {
            format!("Added '{}'.", role.name)
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
        .and_then(|permissions| DiscordPermissions::from_decimal(permissions).ok())
        .is_some_and(|permissions| permissions.contains(DiscordPermissions::ADMINISTRATOR))
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
    use std::{collections::HashMap, sync::Arc};

    use anyhow::Result;

    use super::{has_administrator_permission, CommandRouter};
    use crate::application::{
        ports::{GuildRoleRepository, MemberRoleGateway, RoleMembershipAction, RoleRegistration},
        route::command_data::{ApplicationCommandData, CommandOption, ResolvedData, ResolvedRole},
    };
    use athenaeum::interaction::{Interaction, InteractionKind, Member, User};

    /// CyberSage command interaction carrying the bot's command schema.
    type InteractionRequest = Interaction<ApplicationCommandData>;

    /// Stores interactions made with a fake role-registration repository.
    #[derive(Default)]
    struct FakeRoleRepository {
        /// Roles returned by name lookup and autocomplete.
        roles: Vec<RoleRegistration>,
        /// Successful save operations observed by tests.
        saved_roles: std::sync::Mutex<Vec<(String, RoleRegistration)>>,
    }

    impl GuildRoleRepository for Arc<FakeRoleRepository> {
        async fn query_roles_by_prefix(
            &self,
            _guild_id: &str,
            prefix: &str,
        ) -> Result<Vec<RoleRegistration>> {
            Ok(self
                .roles
                .iter()
                .filter(|role| role.name.to_lowercase().starts_with(&prefix.to_lowercase()))
                .cloned()
                .collect())
        }

        async fn save_role(&self, guild_id: &str, role: &RoleRegistration) -> Result<()> {
            self.saved_roles
                .lock()
                .expect("fake repository lock should not be poisoned")
                .push((guild_id.to_string(), role.clone()));
            Ok(())
        }

        async fn get_role_by_name(
            &self,
            _guild_id: &str,
            role_name: &str,
        ) -> Result<Option<RoleRegistration>> {
            Ok(self
                .roles
                .iter()
                .find(|role| role.name.eq_ignore_ascii_case(role_name))
                .cloned())
        }
    }

    /// Records Discord role operations without making HTTP requests.
    #[derive(Default)]
    struct FakeMemberRoleGateway {
        /// Role IDs returned for the invoking member.
        member_roles: Vec<String>,
        /// Membership changes requested by the command handler.
        changes: std::sync::Mutex<Vec<(String, String, String, RoleMembershipAction)>>,
        /// Whether the fake bot can manage a requested role.
        can_manage_roles: bool,
    }

    impl MemberRoleGateway for Arc<FakeMemberRoleGateway> {
        async fn can_manage_role(&self, _guild_id: &str, _role_id: &str) -> Result<bool> {
            Ok(self.can_manage_roles)
        }

        async fn fetch_member_roles(&self, _guild_id: &str, _user_id: &str) -> Result<Vec<String>> {
            Ok(self.member_roles.clone())
        }

        async fn modify_user_role(
            &self,
            guild_id: &str,
            user_id: &str,
            role_id: &str,
            action: RoleMembershipAction,
        ) -> Result<()> {
            self.changes
                .lock()
                .expect("fake gateway lock should not be poisoned")
                .push((
                    guild_id.to_string(),
                    user_id.to_string(),
                    role_id.to_string(),
                    action,
                ));
            Ok(())
        }
    }

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

    /// Confirms that unauthorized role saves never reach persistence.
    #[tokio::test]
    async fn rejects_unauthorized_role_save() {
        let repository = Arc::new(FakeRoleRepository::default());
        let router = CommandRouter::new(
            repository.clone(),
            Arc::new(FakeMemberRoleGateway {
                can_manage_roles: true,
                ..Default::default()
            }),
        );

        let response = router
            .handle_command(&save_interaction(None))
            .await
            .expect("unauthorized save should produce a response");

        assert_eq!(
            response.data.and_then(|data| data.content).as_deref(),
            Some("Administrator permission is required to register roles.")
        );
        assert!(repository
            .saved_roles
            .lock()
            .expect("fake repository lock should not be poisoned")
            .is_empty());
    }

    /// Confirms that administrator role saves persist Discord's resolved role identity.
    #[tokio::test]
    async fn saves_resolved_role_for_administrator() {
        let repository = Arc::new(FakeRoleRepository::default());
        let router = CommandRouter::new(
            repository.clone(),
            Arc::new(FakeMemberRoleGateway {
                can_manage_roles: true,
                ..Default::default()
            }),
        );

        let response = router
            .handle_command(&save_interaction(Some("8")))
            .await
            .expect("administrator save should succeed");

        assert_eq!(
            response.data.and_then(|data| data.content).as_deref(),
            Some("Role registered successfully.")
        );
        assert_eq!(
            repository
                .saved_roles
                .lock()
                .expect("fake repository lock should not be poisoned")
                .as_slice(),
            [("guild".to_string(), role_registration())]
        );
    }

    /// Confirms that a role the bot cannot manage is not persisted.
    #[tokio::test]
    async fn rejects_role_the_bot_cannot_manage() {
        let repository = Arc::new(FakeRoleRepository::default());
        let router = CommandRouter::new(
            repository.clone(),
            Arc::new(FakeMemberRoleGateway::default()),
        );

        let response = router
            .handle_command(&save_interaction(Some("8")))
            .await
            .expect("unmanageable role should produce a response");

        assert_eq!(
            response.data.and_then(|data| data.content).as_deref(),
            Some("I need Manage Roles permission and a bot role above this role before I can register it.")
        );
        assert!(repository
            .saved_roles
            .lock()
            .expect("fake repository lock should not be poisoned")
            .is_empty());
    }

    /// Confirms that toggling an absent role delegates an add operation to Discord.
    #[tokio::test]
    async fn adds_role_when_member_does_not_have_it() {
        let repository = Arc::new(FakeRoleRepository {
            roles: vec![role_registration()],
            ..Default::default()
        });
        let gateway = Arc::new(FakeMemberRoleGateway::default());
        let router = CommandRouter::new(repository, gateway.clone());

        let response = router
            .handle_command(&toggle_interaction())
            .await
            .expect("toggle should succeed");

        assert_eq!(
            response.data.and_then(|data| data.content).as_deref(),
            Some("Added 'Moderator'.")
        );
        assert_eq!(
            gateway
                .changes
                .lock()
                .expect("fake gateway lock should not be poisoned")
                .as_slice(),
            [(
                "guild".to_string(),
                "user".to_string(),
                "role-id".to_string(),
                RoleMembershipAction::Add,
            )]
        );
    }

    /// Confirms that autocomplete returns matching registered role names.
    #[tokio::test]
    async fn returns_matching_autocomplete_choices() {
        let router = CommandRouter::new(
            Arc::new(FakeRoleRepository {
                roles: vec![role_registration()],
                ..Default::default()
            }),
            Arc::new(FakeMemberRoleGateway::default()),
        );

        let response = router
            .handle_autocomplete(&autocomplete_interaction("mod"))
            .await
            .expect("autocomplete should succeed");

        assert_eq!(
            response
                .data
                .and_then(|data| data.choices)
                .expect("autocomplete response should contain choices")[0]
                .name,
            "Moderator"
        );
    }

    /// Confirms that an unrecognized command does not invoke dependencies.
    #[tokio::test]
    async fn rejects_unknown_command() {
        let router = CommandRouter::new(
            Arc::new(FakeRoleRepository::default()),
            Arc::new(FakeMemberRoleGateway::default()),
        );
        let interaction = InteractionRequest {
            data: Some(ApplicationCommandData {
                name: "unknown".to_string(),
                options: vec![],
                resolved: None,
            }),
            ..interaction_with_permissions(None)
        };

        let response = router
            .handle_command(&interaction)
            .await
            .expect("unknown command should produce a response");

        assert_eq!(
            response.data.and_then(|data| data.content).as_deref(),
            Some("Unknown command.")
        );
    }

    /// Confirms that a member who already holds a registered role has it removed.
    #[tokio::test]
    async fn removes_role_when_member_already_has_it() {
        let repository = Arc::new(FakeRoleRepository {
            roles: vec![role_registration()],
            ..Default::default()
        });
        let gateway = Arc::new(FakeMemberRoleGateway {
            member_roles: vec!["role-id".to_string()],
            ..Default::default()
        });
        let router = CommandRouter::new(repository, gateway.clone());

        let response = router
            .handle_command(&toggle_interaction())
            .await
            .expect("toggle should succeed");

        assert_eq!(
            response.data.and_then(|data| data.content).as_deref(),
            Some("Removed 'Moderator'.")
        );
        assert_eq!(
            gateway
                .changes
                .lock()
                .expect("fake gateway lock should not be poisoned")[0]
                .3,
            RoleMembershipAction::Remove
        );
    }

    /// Builds the minimal guild interaction required for permission checks.
    fn interaction_with_permissions(permissions: Option<&str>) -> InteractionRequest {
        InteractionRequest {
            id: None,
            application_id: None,
            token: None,
            kind: InteractionKind::ApplicationCommand,
            data: None,
            guild_id: Some("guild".to_string()),
            channel_id: None,
            member: Some(Member {
                permissions: permissions.map(str::to_string),
                user: User {
                    id: "user".to_string(),
                },
            }),
        }
    }

    /// Builds a save command with a resolved Discord role.
    fn save_interaction(permissions: Option<&str>) -> InteractionRequest {
        let mut roles = HashMap::new();
        roles.insert(
            "role-id".to_string(),
            ResolvedRole {
                name: "Moderator".to_string(),
            },
        );

        InteractionRequest {
            data: Some(ApplicationCommandData {
                name: "role".to_string(),
                options: vec![CommandOption {
                    name: "save".to_string(),
                    value: None,
                    options: vec![role_option("role-id")],
                }],
                resolved: Some(ResolvedData { roles }),
            }),
            ..interaction_with_permissions(permissions)
        }
    }

    /// Builds a toggle command for the shared test role.
    fn toggle_interaction() -> InteractionRequest {
        InteractionRequest {
            data: Some(ApplicationCommandData {
                name: "role".to_string(),
                options: vec![CommandOption {
                    name: "toggle".to_string(),
                    value: None,
                    options: vec![role_option("Moderator")],
                }],
                resolved: None,
            }),
            ..interaction_with_permissions(None)
        }
    }

    /// Builds an autocomplete interaction for a role-name prefix.
    fn autocomplete_interaction(prefix: &str) -> InteractionRequest {
        InteractionRequest {
            kind: InteractionKind::ApplicationCommandAutocomplete,
            data: Some(ApplicationCommandData {
                name: "role".to_string(),
                options: vec![CommandOption {
                    name: "toggle".to_string(),
                    value: None,
                    options: vec![role_option(prefix)],
                }],
                resolved: None,
            }),
            ..interaction_with_permissions(None)
        }
    }

    /// Builds a string-valued command option.
    fn role_option(value: &str) -> CommandOption {
        CommandOption {
            name: "role".to_string(),
            value: Some(serde_json::Value::String(value.to_string())),
            options: vec![],
        }
    }

    /// Returns the role used by persistence and toggle tests.
    fn role_registration() -> RoleRegistration {
        RoleRegistration {
            name: "Moderator".to_string(),
            id: "role-id".to_string(),
        }
    }
}
