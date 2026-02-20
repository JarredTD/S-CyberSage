use serde::Serialize;
use serde_repr::Serialize_repr;

bitflags::bitflags! {
    pub struct MessageFlags: u64 {
        const EPHEMERAL = 1 << 6;
    }
}

#[derive(Debug, Copy, Clone, Serialize_repr)]
#[repr(u8)]
pub enum InteractionCallbackType {
    Pong = 1,
    ChannelMessageWithSource = 4,
    ApplicationCommandAutocompleteResult = 8,
}

#[derive(Debug, Serialize)]
pub struct InteractionResponse {
    #[serde(rename = "type")]
    pub kind: InteractionCallbackType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<InteractionCallbackData>,
}

#[derive(Debug, Serialize)]
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

impl InteractionResponse {
    pub fn pong() -> Self {
        Self {
            kind: InteractionCallbackType::Pong,
            data: None,
        }
    }

    pub fn message(content: impl Into<String>) -> Self {
        Self {
            kind: InteractionCallbackType::ChannelMessageWithSource,
            data: Some(InteractionCallbackData {
                content: Some(content.into()),
                flags: None,
                choices: None,
            }),
        }
    }

    pub fn ephemeral(content: impl Into<String>) -> Self {
        Self {
            kind: InteractionCallbackType::ChannelMessageWithSource,
            data: Some(InteractionCallbackData {
                content: Some(content.into()),
                flags: Some(MessageFlags::EPHEMERAL.bits()),
                choices: None,
            }),
        }
    }

    pub fn autocomplete(choices: Vec<ApplicationCommandOptionChoice>) -> Self {
        Self {
            kind: InteractionCallbackType::ApplicationCommandAutocompleteResult,
            data: Some(InteractionCallbackData {
                content: None,
                flags: None,
                choices: Some(choices),
            }),
        }
    }
}
