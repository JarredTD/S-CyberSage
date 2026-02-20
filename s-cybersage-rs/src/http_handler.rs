use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_secretsmanager::Client as SecretsClient;
use lambda_http::{Body, Error, Request, Response};
use serde_json::json;
use tokio::sync::OnceCell;

use crate::{
    bal::{
        auth::verify::AuthManager,
        discord::role_manager::RoleManager,
        route::{command_router::CommandRouter, interaction_router::InteractionRouter},
        subscription::SubscriptionManager,
    },
    dal::{
        dao::{guild_dao::GuildDao, payment_dao::PaymentDao},
        model::interaction_request::InteractionRequest,
        reader::secrets_reader::SecretsReader,
    },
};

static DISCORD_PUBLIC_KEY_CACHE: OnceCell<serde_json::Value> = OnceCell::const_new();
static DISCORD_TOKEN_CACHE: OnceCell<serde_json::Value> = OnceCell::const_new();

pub(crate) async fn function_handler(
    event: Request,
    dynamo_client: DynamoClient,
    secrets_client: SecretsClient,
    http_client: reqwest::Client,
) -> Result<Response<Body>, Error> {
    let body_bytes = event.body().as_ref();
    let body_str = std::str::from_utf8(body_bytes).unwrap_or("");

    let headers = event.headers();

    let signature = headers
        .get("x-signature-ed25519")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let timestamp = headers
        .get("x-signature-timestamp")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let secrets_reader = SecretsReader::new(secrets_client.clone());

    let public_key_secret_arn = match std::env::var("DISCORD_PUBLIC_KEY_SECRET_ARN") {
        Ok(v) => v,
        Err(_) => return Ok(server_error()),
    };

    let discord_public_key = match secrets_reader
        .get_secret_value(&public_key_secret_arn, "key", &DISCORD_PUBLIC_KEY_CACHE)
        .await
    {
        Ok(v) => v,
        Err(_) => return Ok(server_error()),
    };

    let subscription_table = match std::env::var("GUILD_SUBSCRIPTIONS_TABLE_NAME") {
        Ok(v) => v,
        Err(_) => return Ok(server_error()),
    };

    let payment_dao = PaymentDao::new(dynamo_client.clone(), subscription_table);
    let subscription_manager = SubscriptionManager::new(payment_dao);

    let auth_manager = AuthManager::new(subscription_manager.clone());

    if auth_manager
        .verify_signature(signature, timestamp, body_bytes, &discord_public_key)
        .is_err()
    {
        return Ok(json_response(
            401,
            &json!({ "error": "Invalid request signature" }),
        ));
    }

    let interaction: InteractionRequest = match serde_json::from_str(body_str) {
        Ok(i) => i,
        Err(_) => return Ok(json_response(400, &json!({ "error": "Invalid JSON" }))),
    };

    let guild_id = match interaction.guild_id.as_deref() {
        Some(id) => id,
        None => return Ok(ephemeral_response("Guild ID missing.")),
    };

    if !subscription_manager
        .is_active(guild_id)
        .await
        .unwrap_or(false)
    {
        return Ok(ephemeral_response(
            "This guild does not have an active subscription.",
        ));
    }

    let role_table = match std::env::var("ROLE_MAPPINGS_TABLE_NAME") {
        Ok(v) => v,
        Err(_) => return Ok(server_error()),
    };

    let guild_dao = GuildDao::new(dynamo_client.clone(), role_table);

    let token_secret_arn = match std::env::var("DISCORD_TOKEN_SECRET_ARN") {
        Ok(v) => v,
        Err(_) => return Ok(server_error()),
    };

    let discord_token = match secrets_reader
        .get_secret_value(&token_secret_arn, "token", &DISCORD_TOKEN_CACHE)
        .await
    {
        Ok(v) => v,
        Err(_) => return Ok(server_error()),
    };

    let role_manager = RoleManager::new(http_client.clone(), discord_token);

    let command_router = CommandRouter::new(guild_dao, role_manager, subscription_manager);

    let interaction_router = InteractionRouter::new(command_router);

    let response = match interaction_router.route(&interaction).await {
        Ok(r) => r,
        Err(_) => crate::dal::model::interaction_response::InteractionResponse::ephemeral(
            "Internal error.",
        ),
    };

    Ok(json_response(200, &response))
}

fn server_error() -> Response<Body> {
    json_response(500, &json!({ "error": "Server misconfiguration" }))
}

fn json_response<T: serde::Serialize>(status: u16, body: &T) -> Response<Body> {
    let body_str = serde_json::to_string(body).unwrap_or_else(|_| "{}".to_string());

    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body_str.into())
        .unwrap()
}

fn ephemeral_response(content: &str) -> Response<Body> {
    json_response(
        200,
        &crate::dal::model::interaction_response::InteractionResponse::ephemeral(content),
    )
}
