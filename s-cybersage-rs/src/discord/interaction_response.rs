use serde::{ser::Serializer, Serialize};

#[derive(Debug, Copy, Clone)]
pub enum InteractionCallbackType {
    Pong = 1,
    ChannelMessageWithSource = 4,
    ApplicationCommandAutocompleteResult = 8,
}

impl Serialize for InteractionCallbackType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct InteractionResponse {
    #[serde(rename = "type")]
    pub kind: InteractionCallbackType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<InteractionCallbackData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct InteractionCallbackData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<ApplicationCommandOptionChoice>>,
}

#[derive(Debug, Serialize)]
pub struct ApplicationCommandOptionChoice {
    pub name: String,
    pub value: String,
}
