use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_secretsmanager::Client as SecretsClient;
use lambda_http::{Body, Error, Request, RequestExt, Response};
use serde_json::json;
use tokio::sync::OnceCell;

use crate::{
    app_context::AppContext,
    transport::discord::{
        interaction_request::InteractionRequest, interaction_response::InteractionResponse,
    },
};

/// Lazily initialized services reused by warm Lambda invocations.
static APP_CONTEXT: OnceCell<AppContext> = OnceCell::const_new();

/// Name of Discord's Ed25519 signature header.
const SIG_HEADER: &str = "x-signature-ed25519";
/// Name of Discord's request timestamp header.
const TS_HEADER: &str = "x-signature-timestamp";

/// Validates and handles a Discord HTTP interaction request.
#[tracing::instrument(skip(event, dynamo_client, secrets_client, http_client))]
pub(crate) async fn function_handler(
    event: Request,
    dynamo_client: DynamoClient,
    secrets_client: SecretsClient,
    http_client: reqwest::Client,
) -> Result<Response<Body>, Error> {
    let request_id = extract_request_id(&event);
    tracing::info!(request_id = %request_id, "handling request");

    let ctx = APP_CONTEXT
        .get_or_try_init(|| async {
            AppContext::new(dynamo_client, secrets_client, http_client).await
        })
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to initialize app context");
            Error::from("Failed to initialize app")
        })?;

    let body_bytes = event.body().as_ref();

    let body_str = match std::str::from_utf8(body_bytes) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "invalid utf8 body");
            return Ok(json_response(
                400,
                &json!({ "error": "Invalid request body" }),
            ));
        }
    };

    let headers = event.headers();

    let signature = headers
        .get(SIG_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let timestamp = headers
        .get(TS_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if let Err(e) =
        ctx.auth_manager
            .verify_signature(signature, timestamp, body_bytes, &ctx.discord_public_key)
    {
        tracing::warn!(error = %e, "signature verification failed");
        return Ok(json_response(
            401,
            &json!({ "error": "Invalid request signature" }),
        ));
    }

    let interaction: InteractionRequest = match serde_json::from_str(body_str) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse interaction json");
            return Ok(json_response(400, &json!({ "error": "Invalid JSON" })));
        }
    };

    tracing::debug!(interaction_type = ?interaction.interaction_type);

    let response = match ctx.interaction_router.route(&interaction).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "interaction routing failed");
            InteractionResponse::ephemeral("Internal error.")
        }
    };

    Ok(json_response(200, &response))
}

/// Serializes a value into a JSON HTTP response with the supplied status code.
fn json_response<T: serde::Serialize>(status: u16, body: &T) -> Response<Body> {
    let body_str = serde_json::to_string(body).unwrap_or_else(|_| "{}".to_string());

    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body_str.into())
        .unwrap()
}

/// Extracts the API Gateway request ID for structured logging.
fn extract_request_id(event: &Request) -> String {
    match event.request_context() {
        lambda_http::request::RequestContext::ApiGatewayV2(ctx) => ctx.request_id.clone(),
        lambda_http::request::RequestContext::ApiGatewayV1(ctx) => ctx.request_id.clone(),
        _ => None,
    }
    .unwrap_or_else(|| "unknown".to_string())
}
