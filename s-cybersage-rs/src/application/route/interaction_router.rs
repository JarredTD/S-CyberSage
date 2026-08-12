use anyhow::Result;

use crate::transport::discord::{
    interaction_request::{InteractionRequest, InteractionType},
    interaction_response::InteractionResponse,
};

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
    pub async fn route(&self, interaction: &InteractionRequest) -> Result<InteractionResponse> {
        match interaction.interaction_type {
            InteractionType::Ping => Ok(InteractionResponse::pong()),
            InteractionType::ApplicationCommandAutocomplete => {
                self.command_router.handle_autocomplete(interaction).await
            }
            InteractionType::ApplicationCommand => {
                self.command_router.handle_command(interaction).await
            }
            InteractionType::Unknown => Ok(InteractionResponse::ephemeral(
                "Unsupported interaction type.",
            )),
        }
    }
}

/// Tests interaction dispatch without infrastructure adapters.
#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::InteractionRouter;
    use crate::{
        application::{
            ports::{
                GuildRoleRepository, MemberRoleGateway, RoleMembershipAction, RoleRegistration,
            },
            route::command_router::CommandRouter,
        },
        transport::discord::{
            interaction_request::{InteractionRequest, InteractionType},
            interaction_response::InteractionCallbackType,
        },
    };

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
            Ok(None)
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
            .route(&interaction(InteractionType::Ping))
            .await
            .expect("ping should not fail");

        assert!(matches!(response.kind, InteractionCallbackType::Pong));
    }

    /// Confirms that unsupported interactions receive an ephemeral response.
    #[tokio::test]
    async fn rejects_unknown_interaction() {
        let response = router()
            .route(&interaction(InteractionType::Unknown))
            .await
            .expect("unsupported interaction should produce a response");

        assert_eq!(
            response.data.and_then(|data| data.content).as_deref(),
            Some("Unsupported interaction type.")
        );
    }

    /// Builds the router with inert dependencies for pure dispatch tests.
    fn router() -> InteractionRouter<NoopRepository, NoopMemberRoleGateway> {
        InteractionRouter::new(CommandRouter::new(NoopRepository, NoopMemberRoleGateway))
    }

    /// Builds the minimal interaction required to test its type-based routing.
    fn interaction(interaction_type: InteractionType) -> InteractionRequest {
        InteractionRequest {
            id: None,
            application_id: None,
            token: None,
            interaction_type,
            data: None,
            guild_id: None,
            member: None,
        }
    }
}
