//! HTTP entry point for the S-CyberSage Discord interaction handler.

use lambda_http::Error;

/// Starts the S-CyberSage Lambda executable.
#[tokio::main]
async fn main() -> Result<(), Error> {
    s_cybersage_rs::run_lambda().await
}
