use lambda_http::{run, service_fn, Error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod auth;
pub mod aws;
pub mod discord;
pub mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let shared_config = aws_config::load_from_env().await;
    let dynamo_client = aws_sdk_dynamodb::Client::new(&shared_config);
    let secrets_client = aws_sdk_secretsmanager::Client::new(&shared_config);

    run(service_fn(move |event| {
        http_handler::function_handler(event, dynamo_client.clone(), secrets_client.clone())
    }))
    .await
}
