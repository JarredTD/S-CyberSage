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
    /// Base URL for Discord interaction webhook requests.
    api_base_url: String,
}

impl InteractionResponder {
    /// Creates an interaction responder with the supplied HTTP client.
    ///
    /// # Arguments
    ///
    /// * `client` - Reusable HTTP client for Discord interaction webhook calls.
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            api_base_url: DISCORD_API_BASE.to_string(),
        }
    }

    /// Creates a responder that targets a custom Discord-compatible REST endpoint.
    ///
    /// # Arguments
    ///
    /// * `client` - Reusable HTTP client for interaction webhook calls.
    /// * `api_base_url` - Base URL of the Discord-compatible REST API.
    pub fn with_api_base_url(client: reqwest::Client, api_base_url: impl Into<String>) -> Self {
        let mut responder = Self::new(client);
        responder.api_base_url = api_base_url.into().trim_end_matches('/').to_string();
        responder
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
        let endpoint = format!(
            "{}/interactions/{interaction_id}/{token}/callback",
            self.api_base_url
        );

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
        let endpoint = format!(
            "{}/webhooks/{application_id}/{token}/messages/@original",
            self.api_base_url
        );

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

/// Tests Discord interaction webhook requests against a local HTTP server.
#[cfg(test)]
mod tests {
    use super::InteractionResponder;
    use crate::transport::discord::{
        interaction_request::{InteractionRequest, InteractionType},
        interaction_response::InteractionResponse,
    };
    use wiremock::{
        matchers::{body_json, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    /// Confirms that command deferral uses Discord's callback endpoint and payload.
    #[tokio::test]
    async fn defers_ephemeral_interaction() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/interactions/interaction/token/callback"))
            .and(body_json(
                serde_json::json!({ "type": 5, "data": { "flags": 64 } }),
            ))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let responder =
            InteractionResponder::with_api_base_url(reqwest::Client::new(), server.uri());

        responder
            .defer_ephemeral(&interaction())
            .await
            .expect("mocked Discord deferral should succeed");
    }

    /// Confirms that a deferred response is replaced through Discord's original-message endpoint.
    #[tokio::test]
    async fn updates_deferred_interaction() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/webhooks/application/token/messages/@original"))
            .and(body_json(
                serde_json::json!({ "content": "Done", "flags": 64 }),
            ))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let responder =
            InteractionResponder::with_api_base_url(reqwest::Client::new(), server.uri());

        responder
            .update_original_response(&interaction(), &InteractionResponse::ephemeral("Done"))
            .await
            .expect("mocked Discord update should succeed");
    }

    /// Builds an interaction with the identifiers required by Discord webhook endpoints.
    fn interaction() -> InteractionRequest {
        InteractionRequest {
            id: Some("interaction".to_string()),
            application_id: Some("application".to_string()),
            token: Some("token".to_string()),
            interaction_type: InteractionType::ApplicationCommand,
            data: None,
            guild_id: None,
            member: None,
        }
    }
}
