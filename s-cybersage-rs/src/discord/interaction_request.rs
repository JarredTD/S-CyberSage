use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum InteractionType {
    Ping = 1,
    ApplicationCommand = 2,
    ApplicationCommandAutocomplete = 4,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InteractionData {
    ApplicationCommand(ApplicationCommandData),
    None,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InteractionRequest {
    pub id: String,
    #[serde(rename = "application_id")]
    pub application_id: String,
    #[serde(rename = "type")]
    pub interaction_type: InteractionType,
    #[serde(default)]
    pub data: Option<InteractionData>,
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub member: Option<Member>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApplicationCommandData {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub options: Option<Vec<CommandOption>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandOption {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Member {
    pub user: User,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
}
