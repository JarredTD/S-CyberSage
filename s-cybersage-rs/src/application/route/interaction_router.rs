use anyhow::Result;
use athenaeum::interaction::{Interaction, InteractionKind, InteractionResponse};

use super::command_data::ApplicationCommandData;
use super::command_router::CommandRouter;
use crate::application::ports::{GuildRoleRepository, MemberRoleGateway};

/// Dispatches Discord interactions to command or autocomplete handlers.
pub struct InteractionRouter<R, G> {
    /// Handles application command behavior.
    command_router: CommandRouter<R, G>,
}

impl<R, G> InteractionRouter<R, G> {
    /// Creates an interaction router backed by a command router.
    ///
    /// # Arguments
    ///
    /// * `command_router` - Handler used for application commands and autocomplete requests.
    pub fn new(command_router: CommandRouter<R, G>) -> Self {
        Self { command_router }
    }
}

impl<R, G> InteractionRouter<R, G>
where
    R: GuildRoleRepository,
    G: MemberRoleGateway,
{
    /// Routes a Discord interaction according to its interaction type.
    ///
    /// # Arguments
    ///
    /// * `interaction` - Verified Discord interaction to process.
    ///
    /// # Errors
    ///
    /// Returns an error when the delegated command handler cannot read or update its backing
    /// services.
    #[tracing::instrument(skip(self, interaction))]
    pub async fn route(
        &self,
        interaction: &Interaction<ApplicationCommandData>,
    ) -> Result<InteractionResponse> {
        match interaction.kind {
            InteractionKind::Ping => Ok(InteractionResponse::pong()),
            InteractionKind::ApplicationCommandAutocomplete => {
                self.command_router.handle_autocomplete(interaction).await
            }
            InteractionKind::ApplicationCommand => {
                self.command_router.handle_command(interaction).await
            }
            InteractionKind::MessageComponent
            | InteractionKind::ModalSubmit
            | InteractionKind::Unknown => Ok(InteractionResponse::ephemeral(
                "Unsupported interaction type.",
            )),
        }
    }
}

/// Tests interaction dispatch without infrastructure adapters.
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anyhow::Result;

    use super::InteractionRouter;
    use crate::application::{
        ports::{GuildRoleRepository, MemberRoleGateway, RoleMembershipAction, RoleRegistration},
        route::{
            command_data::{ApplicationCommandData, CommandOption, ResolvedData, ResolvedRole},
            command_router::CommandRouter,
        },
    };
    use athenaeum::interaction::{
        Interaction, InteractionCallbackType, InteractionKind, Member, User,
    };

    /// CyberSage command interaction carrying the bot's command schema.
    type InteractionRequest = Interaction<ApplicationCommandData>;

    /// Provides an inert role repository for routes that do not access storage.
    struct NoopRepository;

    impl GuildRoleRepository for NoopRepository {
        async fn query_roles_by_prefix(
            &self,
            _guild_id: &str,
            _prefix: &str,
        ) -> Result<Vec<RoleRegistration>> {
            Ok(vec![])
        }

        async fn save_role(&self, _guild_id: &str, _role: &RoleRegistration) -> Result<()> {
            Ok(())
        }

        async fn get_role_by_name(
            &self,
            _guild_id: &str,
            _role_name: &str,
        ) -> Result<Option<RoleRegistration>> {
            Ok(Some(RoleRegistration {
                id: "role-id".to_string(),
                name: "Moderator".to_string(),
            }))
        }
    }

    /// Provides an inert Discord gateway for routes that do not call Discord.
    struct NoopMemberRoleGateway;

    impl MemberRoleGateway for NoopMemberRoleGateway {
        async fn can_manage_role(&self, _guild_id: &str, _role_id: &str) -> Result<bool> {
            Ok(true)
        }

        async fn fetch_member_roles(&self, _guild_id: &str, _user_id: &str) -> Result<Vec<String>> {
            Ok(vec![])
        }

        async fn modify_user_role(
            &self,
            _guild_id: &str,
            _user_id: &str,
            _role_id: &str,
            _action: RoleMembershipAction,
        ) -> Result<()> {
            Ok(())
        }
    }

    /// Confirms that Discord verification pings receive a synchronous pong response.
    #[tokio::test]
    async fn responds_to_ping() {
        let response = router()
            .route(&interaction(InteractionKind::Ping))
            .await
            .expect("ping should not fail");

        assert!(matches!(response.kind, InteractionCallbackType::Pong));
    }

    /// Confirms that unsupported interactions receive an ephemeral response.
    #[tokio::test]
    async fn rejects_unknown_interaction() {
        for kind in [
            InteractionKind::MessageComponent,
            InteractionKind::ModalSubmit,
            InteractionKind::Unknown,
        ] {
            let response = router()
                .route(&interaction(kind))
                .await
                .expect("unsupported interaction should produce a response");

            assert_eq!(
                response.data.and_then(|data| data.content).as_deref(),
                Some("Unsupported interaction type.")
            );
        }
    }

    /// Delegates autocomplete interactions to the command router.
    #[tokio::test]
    async fn routes_autocomplete_interaction() {
        let mut request = interaction(InteractionKind::ApplicationCommandAutocomplete);
        request.guild_id = Some("guild".to_string());

        let response = router()
            .route(&request)
            .await
            .expect("autocomplete should route");

        assert!(matches!(
            response.kind,
            InteractionCallbackType::ApplicationCommandAutocompleteResult
        ));
    }

    /// Delegates application commands to the command router.
    #[tokio::test]
    async fn routes_application_command() {
        let mut request = interaction(InteractionKind::ApplicationCommand);
        request.guild_id = Some("guild".to_string());
        request.data = Some(ApplicationCommandData {
            name: "unknown".to_string(),
            options: vec![],
            resolved: None,
        });

        let response = router()
            .route(&request)
            .await
            .expect("command should route");

        assert_eq!(
            response.data.and_then(|data| data.content).as_deref(),
            Some("Unknown command.")
        );
    }

    /// Routes role registration and membership changes through every application port.
    #[tokio::test]
    async fn routes_role_commands() {
        let mut toggle = interaction(InteractionKind::ApplicationCommand);
        toggle.guild_id = Some("guild".to_string());
        toggle.member = Some(member(None));
        toggle.data = Some(role_command("toggle", "Moderator", None));
        let toggle_response = router().route(&toggle).await.expect("toggle should route");
        assert_eq!(
            toggle_response
                .data
                .and_then(|data| data.content)
                .as_deref(),
            Some("Added 'Moderator'.")
        );

        let mut roles = HashMap::new();
        roles.insert(
            "role-id".to_string(),
            ResolvedRole {
                name: "Moderator".to_string(),
            },
        );
        let mut save = interaction(InteractionKind::ApplicationCommand);
        save.guild_id = Some("guild".to_string());
        save.member = Some(member(Some("8")));
        save.data = Some(role_command(
            "save",
            "role-id",
            Some(ResolvedData { roles }),
        ));
        let save_response = router().route(&save).await.expect("save should route");
        assert_eq!(
            save_response.data.and_then(|data| data.content).as_deref(),
            Some("Role registered successfully.")
        );
    }

    /// Builds the router with inert dependencies for pure dispatch tests.
    fn router() -> InteractionRouter<NoopRepository, NoopMemberRoleGateway> {
        InteractionRouter::new(CommandRouter::new(NoopRepository, NoopMemberRoleGateway))
    }

    /// Builds the minimal interaction required to test its type-based routing.
    fn interaction(kind: InteractionKind) -> InteractionRequest {
        InteractionRequest {
            id: None,
            application_id: None,
            token: None,
            kind,
            data: None,
            guild_id: None,
            channel_id: None,
            member: None,
        }
    }

    /// Builds a role command with one string-valued role option.
    fn role_command(
        action: &str,
        value: &str,
        resolved: Option<ResolvedData>,
    ) -> ApplicationCommandData {
        ApplicationCommandData {
            name: "role".to_string(),
            options: vec![CommandOption {
                name: action.to_string(),
                value: None,
                options: vec![CommandOption {
                    name: "role".to_string(),
                    value: Some(serde_json::Value::String(value.to_string())),
                    options: vec![],
                }],
            }],
            resolved,
        }
    }

    /// Builds a guild member with optional Discord permission bits.
    fn member(permissions: Option<&str>) -> Member {
        Member {
            permissions: permissions.map(str::to_string),
            user: User {
                id: "user".to_string(),
            },
        }
    }
}
