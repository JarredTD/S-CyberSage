use lambda_http::{run, service_fn, Error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod auth;
pub mod aws;
pub mod discord;
pub mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    run(service_fn(http_handler::function_handler)).await
}
