use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_secretsmanager::Client as SecretsClient;

use crate::{
    bal::{
        auth::verify::AuthManager,
        discord::role_manager::RoleManager,
        route::{command_router::CommandRouter, interaction_router::InteractionRouter},
    },
    dal::{dao::guild::GuildDao, reader::secrets_reader::SecretsReader},
};

pub struct AppContext {
    pub auth_manager: AuthManager,
    pub interaction_router: InteractionRouter,
    pub discord_public_key: String,
}

impl AppContext {
    pub async fn new(
        dynamo: DynamoClient,
        secrets: SecretsClient,
        http: reqwest::Client,
    ) -> anyhow::Result<Self> {
        let main_table = std::env::var("MAIN_TABLE_NAME")?;
        let public_key_secret_arn = std::env::var("DISCORD_PUBLIC_KEY_SECRET_ARN")?;
        let token_secret_arn = std::env::var("DISCORD_TOKEN_SECRET_ARN")?;

        let secrets_reader = SecretsReader::new(secrets);

        let discord_public_key = secrets_reader
            .get_string(&public_key_secret_arn, "key")
            .await?;

        let discord_token = secrets_reader
            .get_string(&token_secret_arn, "token")
            .await?;

        let guild_dao = GuildDao::new(dynamo, main_table);
        let role_manager = RoleManager::new(http, discord_token);

        let command_router = CommandRouter::new(guild_dao, role_manager);
        let interaction_router = InteractionRouter::new(command_router);

        Ok(Self {
            auth_manager: AuthManager::new(),
            interaction_router,
            discord_public_key,
        })
    }
}
