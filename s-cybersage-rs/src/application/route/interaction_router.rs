use anyhow::Result;

use crate::transport::discord::{
    interaction_request::{InteractionRequest, InteractionType},
    interaction_response::InteractionResponse,
};

use super::command_router::CommandRouter;

/// Dispatches Discord interactions to command or autocomplete handlers.
pub struct InteractionRouter {
    /// Handles application command behavior.
    command_router: CommandRouter,
}

impl InteractionRouter {
    /// Creates an interaction router backed by a command router.
    ///
    /// # Arguments
    ///
    /// * `command_router` - Handler used for application commands and autocomplete requests.
    pub fn new(command_router: CommandRouter) -> Self {
        Self { command_router }
    }

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
