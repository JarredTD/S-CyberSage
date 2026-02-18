use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_secretsmanager::Client as SecretsClient;
use lambda_http::{Body, Error, Request, Response};
use serde_json::json;
use tokio::sync::OnceCell;

use crate::{
    auth::verify::verify_discord_request,
    aws::dynamo_db::RoleDb,
    aws::secrets::SecretsManager,
    discord::interaction_request::{ApplicationCommandData, InteractionRequest, InteractionType},
    discord::interaction_response::{
        ApplicationCommandOptionChoice, InteractionCallbackData, InteractionCallbackType,
        InteractionResponse,
    },
    discord::roles::{fetch_member_roles, modify_user_role, RoleAction},
};

const EPHEMERAL_FLAG: u64 = 1 << 6;

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

    let secrets = SecretsManager::new_with_client(secrets_client.clone());

    let public_key_secret_arn = match std::env::var("DISCORD_PUBLIC_KEY_SECRET_ARN") {
        Ok(v) => v,
        Err(_) => {
            return Ok(json_response(
                500,
                &json!({ "error": "Server misconfiguration" }),
            ))
        }
    };

    let discord_public_key = match secrets
        .get_secret_cached(&public_key_secret_arn, "key", &DISCORD_PUBLIC_KEY_CACHE)
        .await
    {
        Ok(v) => v,
        Err(_) => {
            return Ok(json_response(
                500,
                &json!({ "error": "Server misconfiguration" }),
            ))
        }
    };

    if verify_discord_request(signature, timestamp, body_bytes, &discord_public_key).is_err() {
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

    let role_table = match std::env::var("ROLE_MAPPINGS_TABLE_NAME") {
        Ok(v) => v,
        Err(_) => {
            return Ok(json_response(
                500,
                &json!({ "error": "Server misconfiguration" }),
            ))
        }
    };

    let role_db = RoleDb::new(dynamo_client.clone(), role_table);

    let token_secret_arn = match std::env::var("DISCORD_TOKEN_SECRET_ARN") {
        Ok(v) => v,
        Err(_) => {
            return Ok(json_response(
                500,
                &json!({ "error": "Server misconfiguration" }),
            ))
        }
    };

    let discord_token = match secrets
        .get_secret_cached(&token_secret_arn, "token", &DISCORD_TOKEN_CACHE)
        .await
    {
        Ok(v) => v,
        Err(_) => {
            return Ok(json_response(
                500,
                &json!({ "error": "Server misconfiguration" }),
            ))
        }
    };

    let response = match interaction.interaction_type {
        InteractionType::Ping => InteractionResponse {
            kind: InteractionCallbackType::Pong,
            data: None,
        },

        InteractionType::ApplicationCommandAutocomplete => {
            let prefix = interaction
                .data
                .as_ref()
                .and_then(|cmd| cmd.options.as_ref())
                .and_then(|opts| opts.first())
                .and_then(|sub| sub.options.as_ref())
                .and_then(|sub_opts| sub_opts.first())
                .and_then(|opt| opt.value.as_ref())
                .and_then(|val| val.as_str())
                .unwrap_or("");

            let roles = role_db
                .query_roles_by_prefix(guild_id, prefix)
                .await
                .unwrap_or_default();

            let choices: Vec<ApplicationCommandOptionChoice> = roles
                .into_iter()
                .map(|(role_name, _role_id)| ApplicationCommandOptionChoice {
                    name: role_name.clone(),
                    value: role_name,
                })
                .collect();

            InteractionResponse {
                kind: InteractionCallbackType::ApplicationCommandAutocompleteResult,
                data: Some(InteractionCallbackData {
                    content: None,
                    flags: None,
                    choices: Some(choices),
                }),
            }
        }

        InteractionType::ApplicationCommand => {
            let cmd_data: &ApplicationCommandData = match interaction.data.as_ref() {
                Some(d) => d,
                None => return Ok(ephemeral_response("Invalid command data.")),
            };

            if cmd_data.name != "role" {
                return Ok(ephemeral_response("Unknown command."));
            }

            let subcommand = match cmd_data.options.as_ref().and_then(|o| o.first()) {
                Some(s) => s,
                None => return Ok(ephemeral_response("Missing subcommand.")),
            };

            match subcommand.name.as_str() {
                "save" => {
                    let role_id = subcommand
                        .options
                        .as_ref()
                        .and_then(|opts| opts.first())
                        .and_then(|opt| opt.value.as_ref())
                        .and_then(|val| val.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    if role_id.is_empty() {
                        return Ok(ephemeral_response("Role is required."));
                    }

                    let role_name = match cmd_data
                        .resolved
                        .as_ref()
                        .and_then(|r| r.roles.get(&role_id))
                        .map(|r| r.name.clone())
                    {
                        Some(n) => n,
                        None => return Ok(ephemeral_response("Resolved role was missing.")),
                    };

                    if role_db
                        .save_role(guild_id, &role_id, &role_name)
                        .await
                        .is_err()
                    {
                        return Ok(ephemeral_response("Failed to register role."));
                    }

                    return Ok(ephemeral_response("Role registered successfully."));
                }

                "toggle" => {
                    let role_name_input = subcommand
                        .options
                        .as_ref()
                        .and_then(|opts| opts.first())
                        .and_then(|opt| opt.value.as_ref())
                        .and_then(|val| val.as_str())
                        .unwrap_or("")
                        .to_string();

                    if role_name_input.is_empty() {
                        return Ok(ephemeral_response("Role is required."));
                    }

                    let (role_name, role_id) = match role_db
                        .get_role_by_name(guild_id, &role_name_input)
                        .await
                    {
                        Ok(Some(role)) => role,
                        _ => return Ok(ephemeral_response("That role is not self-assignable.")),
                    };

                    let user_id = match interaction.member.as_ref() {
                        Some(m) => &m.user.id,
                        None => return Ok(ephemeral_response("User missing.")),
                    };

                    let member_roles =
                        match fetch_member_roles(&http_client, &discord_token, guild_id, user_id)
                            .await
                        {
                            Ok(r) => r,
                            Err(_) => return Ok(ephemeral_response("Failed to fetch your roles.")),
                        };

                    let has_role = member_roles.iter().any(|r| r == &role_id);

                    let action = if has_role {
                        RoleAction::Remove
                    } else {
                        RoleAction::Add
                    };

                    let result = modify_user_role(
                        &http_client,
                        &discord_token,
                        guild_id,
                        user_id,
                        &role_id,
                        action,
                    )
                    .await;

                    let message = match result {
                        Ok(_) => {
                            if has_role {
                                format!("Removed '{}'.", role_name)
                            } else {
                                format!("Added '{}'.", role_name)
                            }
                        }
                        Err(_) => "Failed to modify role.".to_string(),
                    };

                    InteractionResponse {
                        kind: InteractionCallbackType::ChannelMessageWithSource,
                        data: Some(InteractionCallbackData {
                            content: Some(message),
                            flags: Some(EPHEMERAL_FLAG),
                            choices: None,
                        }),
                    }
                }

                _ => return Ok(ephemeral_response("Unknown subcommand.")),
            }
        }
    };

    Ok(json_response(200, &response))
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
