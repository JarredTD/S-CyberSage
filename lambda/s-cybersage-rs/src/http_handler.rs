use anyhow::Context;
use aws_sdk_dynamodb;
use lambda_http::{Body, Error, Request, Response};
use reqwest;
use serde_json::json;
use tracing;

use crate::{
    auth::verify::verify_discord_request,
    aws::dynamo_db::RoleDb,
    aws::secrets::SecretsManager,
    discord::interaction_request::{InteractionData, InteractionRequest, InteractionType},
    discord::interaction_response::{
        ApplicationCommandOptionChoice, InteractionCallbackData, InteractionCallbackType,
        InteractionResponse,
    },
    discord::roles::{fetch_member_roles, modify_user_role},
};

// Constants
const EPHEMERAL_FLAG: u64 = 1 << 6;

pub(crate) async fn function_handler(event: Request) -> Result<Response<Body>, Error> {
    let headers = event.headers();
    let signature = headers
        .get("x-signature-ed25519")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let timestamp = headers
        .get("x-signature-timestamp")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    let body_bytes = event.body().as_ref();
    let body_str = std::str::from_utf8(body_bytes).unwrap_or("");

    let secrets = SecretsManager::new()
        .await
        .context("Failed to init secrets manager")?;

    let discord_public_key = secrets
        .get_secret(
            &std::env::var("DISCORD_PUBLIC_KEY_SECRET_ARN").unwrap(),
            "key",
        )
        .await
        .context("Failed to get Discord public key")?;

    if let Err(e) = verify_discord_request(signature, timestamp, body_bytes, &discord_public_key) {
        tracing::warn!("Signature verification failed: {:?}", e);
        return Ok(json_response(
            401,
            &json!({ "error": "Invalid request signature" }),
        ));
    }

    let interaction: InteractionRequest = match serde_json::from_str(body_str) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!("Failed to parse interaction request: {:?}", e);
            return Ok(json_response(400, &json!({ "error": "Invalid JSON" })));
        }
    };

    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let config = aws_sdk_dynamodb::Config::builder()
        .region(aws_types::region::Region::new(region))
        .build();
    let dynamo_client = aws_sdk_dynamodb::Client::from_conf(config);

    let role_db = RoleDb::new(
        dynamo_client,
        std::env::var("ROLE_MAPPINGS_TABLE_NAME").unwrap(),
    );

    let discord_token = secrets
        .get_secret(&std::env::var("DISCORD_TOKEN_SECRET_ARN").unwrap(), "token")
        .await
        .context("Failed to get Discord token")?;

    match interaction.interaction_type {
        InteractionType::Ping => {
            let resp = InteractionResponse {
                kind: InteractionCallbackType::Pong,
                data: None,
            };
            Ok(json_response(200, &resp))
        }

        InteractionType::ApplicationCommandAutocomplete => {
            let prefix = interaction
                .data
                .as_ref()
                .and_then(|d| match d {
                    InteractionData::ApplicationCommand(cmd) => {
                        cmd.options.as_ref().and_then(|opts| opts.first())
                    }
                    InteractionData::None => None,
                })
                .and_then(|opt| opt.value.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("");

            let roles = role_db
                .scan_roles_by_prefix(prefix)
                .await
                .unwrap_or_default();

            let choices: Vec<ApplicationCommandOptionChoice> = roles
                .into_iter()
                .map(|role_name| ApplicationCommandOptionChoice {
                    name: role_name.clone(),
                    value: role_name,
                })
                .collect();

            let resp = InteractionResponse {
                kind: InteractionCallbackType::ApplicationCommandAutocompleteResult,
                data: Some(InteractionCallbackData {
                    content: None,
                    flags: None,
                    choices: Some(choices),
                }),
            };

            Ok(json_response(200, &resp))
        }

        InteractionType::ApplicationCommand => {
            let cmd_data = match &interaction.data {
                Some(InteractionData::ApplicationCommand(data)) => data,
                _ => return Ok(ephemeral_response("Invalid command data")),
            };

            if cmd_data.name != "role" {
                return Ok(ephemeral_response("Unknown command"));
            }

            let role_name = match cmd_data.options.as_ref().and_then(|opts| opts.first()) {
                Some(opt) => match &opt.value {
                    Some(val) => val,
                    None => return Ok(ephemeral_response("Role name is missing")),
                },
                None => return Ok(ephemeral_response("Role name is missing")),
            };

            let role_id = match role_db.get_role_id(role_name).await {
                Ok(Some(rid)) => rid,
                Ok(None) => {
                    return Ok(ephemeral_response(&format!(
                        "Role '{}' not found",
                        role_name
                    )))
                }
                Err(_) => return Ok(ephemeral_response("Failed to lookup role")),
            };

            let guild_id = match &interaction.guild_id {
                Some(id) => id,
                None => return Ok(ephemeral_response("Guild ID is missing")),
            };

            let user_id = match &interaction.member {
                Some(member) => &member.user.id,
                None => return Ok(ephemeral_response("User information missing")),
            };

            let client = reqwest::Client::new();
            let member_roles =
                match fetch_member_roles(&client, &discord_token, guild_id, user_id).await {
                    Ok(roles) => roles,
                    Err(_) => return Ok(ephemeral_response("Failed to fetch your roles")),
                };

            let has_role = member_roles.iter().any(|r| r == &role_id);

            let success = if has_role {
                modify_user_role(
                    &client,
                    &discord_token,
                    guild_id,
                    user_id,
                    &role_id,
                    "remove",
                )
                .await
            } else {
                modify_user_role(&client, &discord_token, guild_id, user_id, &role_id, "add").await
            };

            let message = if success {
                if has_role {
                    format!("The '{}' role was removed from you.", role_name)
                } else {
                    format!("You were given the '{}' role.", role_name)
                }
            } else if has_role {
                "Failed to remove role.".to_string()
            } else {
                "Failed to add role.".to_string()
            };

            Ok(ephemeral_response(&message))
        }
    }
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
    let resp = InteractionResponse {
        kind: InteractionCallbackType::ChannelMessageWithSource,
        data: Some(InteractionCallbackData {
            content: Some(content.to_string()),
            flags: Some(EPHEMERAL_FLAG),
            choices: None,
        }),
    };
    json_response(200, &resp)
}
