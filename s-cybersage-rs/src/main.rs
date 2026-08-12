#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! HTTP entry point for the S-CyberSage Discord interaction handler.

use lambda_http::{run, service_fn, Error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Builds the shared dependencies used by request handling.
mod app_context;
/// Contains the interaction routing and authorization use cases.
mod application;
/// Receives and validates HTTP interactions from Discord.
mod http_handler;
/// Provides DynamoDB and Secrets Manager adapters.
mod infrastructure;
/// Defines inbound and outbound protocol representations.
mod transport;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_level(true),
        )
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let shared_config = aws_config::load_from_env().await;

    let dynamo_client = aws_sdk_dynamodb::Client::new(&shared_config);
    let secrets_client = aws_sdk_secretsmanager::Client::new(&shared_config);

    let http_client = reqwest::Client::builder()
        .user_agent("cybersage-bot")
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .pool_max_idle_per_host(5)
        .build()?;

    run(service_fn(move |event| {
        http_handler::function_handler(
            event,
            dynamo_client.clone(),
            secrets_client.clone(),
            http_client.clone(),
        )
    }))
    .await
}
