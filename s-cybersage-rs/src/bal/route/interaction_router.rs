use anyhow::Result;

use crate::dal::model::{
    interaction_request::{InteractionRequest, InteractionType},
    interaction_response::InteractionResponse,
};

use super::command_router::CommandRouter;

pub struct InteractionRouter {
    command_router: CommandRouter,
}

impl InteractionRouter {
    pub fn new(command_router: CommandRouter) -> Self {
        Self { command_router }
    }

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
