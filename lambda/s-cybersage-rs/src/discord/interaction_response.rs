use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InteractionCallbackType {
    Pong = 1,
    ChannelMessageWithSource = 4,
    ApplicationCommandAutocompleteResult = 8,
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
