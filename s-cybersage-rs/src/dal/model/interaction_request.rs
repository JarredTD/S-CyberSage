use serde::Deserialize;
use serde_repr::Deserialize_repr;

#[derive(Debug, Deserialize_repr)]
#[repr(u8)]
pub enum InteractionType {
    Ping = 1,
    ApplicationCommand = 2,
    ApplicationCommandAutocomplete = 4,

    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct InteractionRequest {
    pub id: String,

    #[serde(rename = "application_id")]
    pub application_id: String,

    #[serde(rename = "type")]
    pub interaction_type: InteractionType,

    #[serde(default)]
    pub data: Option<ApplicationCommandData>,

    #[serde(default)]
    pub guild_id: Option<String>,

    #[serde(default)]
    pub member: Option<Member>,
}

#[derive(Debug, Deserialize)]
pub struct ApplicationCommandData {
    pub id: String,
    pub name: String,

    #[serde(default)]
    pub options: Vec<CommandOption>,

    #[serde(default)]
    pub resolved: Option<ResolvedData>,
}

#[derive(Debug, Deserialize)]
pub struct CommandOption {
    pub name: String,

    #[serde(default)]
    pub value: Option<serde_json::Value>,

    #[serde(default)]
    pub options: Vec<CommandOption>,
}

#[derive(Debug, Deserialize)]
pub struct Member {
    pub user: User,

    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolvedData {
    #[serde(default)]
    pub roles: std::collections::HashMap<String, ResolvedRole>,
}

#[derive(Debug, Deserialize)]
pub struct ResolvedRole {
    pub id: String,
    pub name: String,
}
