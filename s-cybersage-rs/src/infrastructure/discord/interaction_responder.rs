use anyhow::{anyhow, Context, Result};

use crate::transport::discord::{
    interaction_request::InteractionRequest, interaction_response::InteractionResponse,
};

/// Base URL for Discord's REST API.
const DISCORD_API_BASE: &str = "https://discord.com/api/v10";

/// Implements deferred and completed responses for Discord interactions.
pub struct InteractionResponder {
    /// HTTP client used to call Discord's interaction webhook endpoints.
    client: reqwest::Client,
}

impl InteractionResponder {
    /// Creates an interaction responder with the supplied HTTP client.
    ///
    /// # Arguments
    ///
    /// * `client` - Reusable HTTP client for Discord interaction webhook calls.
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Acknowledges a command before Discord's three-second interaction deadline.
    ///
    /// # Arguments
    ///
    /// * `interaction` - Signed Discord command interaction containing an ID and response token.
    ///
    /// # Errors
    ///
    /// Returns an error when the interaction lacks callback identifiers or Discord rejects the
    /// deferred acknowledgement.
    pub async fn defer_ephemeral(&self, interaction: &InteractionRequest) -> Result<()> {
        let interaction_id = require_interaction_id(interaction)?;
        let token = require_interaction_token(interaction)?;
        let endpoint = format!("{DISCORD_API_BASE}/interactions/{interaction_id}/{token}/callback");

        self.client
            .post(endpoint)
            .json(&InteractionResponse::deferred_ephemeral())
            .send()
            .await
            .context("Discord interaction acknowledgement request failed")?
            .error_for_status()
            .context("Discord rejected the interaction acknowledgement")?;

        Ok(())
    }

    /// Replaces the deferred interaction message with the completed command result.
    ///
    /// # Arguments
    ///
    /// * `interaction` - Original Discord interaction containing webhook identifiers.
    /// * `response` - Completed message response used to replace the deferred message.
    ///
    /// # Errors
    ///
    /// Returns an error when the interaction is incomplete, the response has no message data, or
    /// Discord rejects the webhook update.
    pub async fn update_original_response(
        &self,
        interaction: &InteractionRequest,
        response: &InteractionResponse,
    ) -> Result<()> {
        let application_id = require_application_id(interaction)?;
        let token = require_interaction_token(interaction)?;
        let data = response
            .data
            .as_ref()
            .ok_or_else(|| anyhow!("Interaction response did not contain message data"))?;
        let endpoint =
            format!("{DISCORD_API_BASE}/webhooks/{application_id}/{token}/messages/@original");

        self.client
            .patch(endpoint)
            .json(data)
            .send()
            .await
            .context("Discord interaction response update request failed")?
            .error_for_status()
            .context("Discord rejected the interaction response update")?;

        Ok(())
    }
}

/// Returns the interaction ID required by Discord's callback endpoint.
fn require_interaction_id(interaction: &InteractionRequest) -> Result<&str> {
    interaction
        .id
        .as_deref()
        .ok_or_else(|| anyhow!("Missing interaction id"))
}

/// Returns the application ID required by Discord's webhook endpoint.
fn require_application_id(interaction: &InteractionRequest) -> Result<&str> {
    interaction
        .application_id
        .as_deref()
        .ok_or_else(|| anyhow!("Missing application id"))
}

/// Returns the token required by Discord's interaction response endpoints.
fn require_interaction_token(interaction: &InteractionRequest) -> Result<&str> {
    interaction
        .token
        .as_deref()
        .ok_or_else(|| anyhow!("Missing interaction token"))
}
