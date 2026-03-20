use std::collections::HashMap;

use serde::Deserialize;
use serde_repr::Deserialize_repr;

#[derive(Debug, Deserialize_repr, Clone, Copy)]
#[repr(u8)]
pub enum InteractionType {
    Ping = 1,
    ApplicationCommand = 2,
    ApplicationCommandAutocomplete = 4,

    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
pub struct ApplicationCommandData {
    pub id: String,
    pub name: String,

    #[serde(default)]
    pub options: Vec<CommandOption>,

    #[serde(default)]
    pub resolved: Option<ResolvedData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CommandOption {
    pub name: String,

    #[serde(default)]
    pub value: Option<serde_json::Value>,

    #[serde(default)]
    pub options: Vec<CommandOption>,
}

impl CommandOption {
    pub fn value_as_str(&self) -> Option<&str> {
        self.value.as_ref()?.as_str()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Member {
    pub user: User,

    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct User {
    pub id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResolvedData {
    #[serde(default)]
    pub roles: HashMap<String, ResolvedRole>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResolvedRole {
    pub id: String,
    pub name: String,
}
