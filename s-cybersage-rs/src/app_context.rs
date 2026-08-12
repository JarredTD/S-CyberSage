use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_secretsmanager::Client as SecretsClient;

use crate::{
    application::{
        auth::verify::AuthManager,
        discord::{interaction_responder::InteractionResponder, role_manager::RoleManager},
        route::{command_router::CommandRouter, interaction_router::InteractionRouter},
    },
    infrastructure::{dao::guild::GuildDao, reader::secrets_reader::SecretsReader},
};

/// Holds initialized services shared across Lambda invocations.
pub struct AppContext {
    /// Verifies signatures on incoming Discord interactions.
    pub auth_manager: AuthManager,
    /// Routes verified interactions to application handlers.
    pub interaction_router: InteractionRouter,
    /// Sends lifecycle responses for commands that exceed Discord's inline deadline.
    pub interaction_responder: InteractionResponder,
    /// Hex-encoded Discord application public key used for verification.
    pub discord_public_key: String,
}

impl AppContext {
    /// Initializes application services from AWS clients and Lambda configuration.
    ///
    /// # Arguments
    ///
    /// * `dynamo` - Client used to persist self-assignable role registrations.
    /// * `secrets` - Client used to retrieve Discord credentials.
    /// * `http` - Reusable client for Discord API requests.
    ///
    /// # Errors
    ///
    /// Returns an error when required environment variables are absent or Discord credentials
    /// cannot be retrieved from Secrets Manager.
    pub async fn new(
        dynamo: DynamoClient,
        secrets: SecretsClient,
        http: reqwest::Client,
    ) -> anyhow::Result<Self> {
        let main_table = std::env::var("MAIN_TABLE_NAME")?;
        let public_key_secret_arn = std::env::var("DISCORD_PUBLIC_KEY_SECRET_ARN")?;
        let token_secret_arn = std::env::var("DISCORD_TOKEN_SECRET_ARN")?;

        let secrets_reader = SecretsReader::new(secrets);

        let (discord_public_key, discord_token) = tokio::try_join!(
            secrets_reader.get_string(&public_key_secret_arn, "key"),
            secrets_reader.get_string(&token_secret_arn, "token")
        )?;

        let guild_dao = GuildDao::new(dynamo, main_table);
        let role_manager = RoleManager::new(http.clone(), discord_token);

        let command_router = CommandRouter::new(guild_dao, role_manager);
        let interaction_router = InteractionRouter::new(command_router);

        Ok(Self {
            auth_manager: AuthManager::new(),
            interaction_router,
            interaction_responder: InteractionResponder::new(http),
            discord_public_key,
        })
    }
}
