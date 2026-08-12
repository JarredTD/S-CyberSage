use serde::Serialize;
use serde_repr::Serialize_repr;

bitflags::bitflags! {
    pub struct MessageFlags: u64 {
        const EPHEMERAL = 1 << 6;
    }
}

/// Identifies the callback protocol response sent to Discord.
#[derive(Debug, Copy, Clone, Serialize_repr)]
#[repr(u8)]
pub enum InteractionCallbackType {
    /// Acknowledges a Discord ping interaction.
    Pong = 1,
    /// Creates an immediate interaction response message.
    ChannelMessageWithSource = 4,
    /// Defers an ephemeral message while command work continues asynchronously.
    DeferredChannelMessageWithSource = 5,
    /// Supplies choices for an autocomplete interaction.
    ApplicationCommandAutocompleteResult = 8,
}

/// Represents a response to a Discord interaction webhook.
#[derive(Debug, Serialize)]
pub struct InteractionResponse {
    /// Callback type that controls Discord's response handling.
    #[serde(rename = "type")]
    pub kind: InteractionCallbackType,

    /// Optional callback payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<InteractionCallbackData>,
}

/// Contains the message or autocomplete data in an interaction response.
#[derive(Debug, Serialize)]
pub struct InteractionCallbackData {
    /// Message text to display to the invoking user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Discord message flags, such as ephemeral visibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u64>,

    /// Choices returned for an autocomplete interaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<ApplicationCommandOptionChoice>>,
}

/// Represents one selectable autocomplete choice.
#[derive(Debug, Serialize)]
pub struct ApplicationCommandOptionChoice {
    /// Human-readable text shown in Discord.
    pub name: String,
    /// Value submitted to the command when selected.
    pub value: String,
}

impl InteractionResponse {
    /// Builds a response that acknowledges a Discord ping.
    pub fn pong() -> Self {
        Self {
            kind: InteractionCallbackType::Pong,
            data: None,
        }
    }

    /// Builds an ephemeral message response visible only to the invoking user.
    ///
    /// # Arguments
    ///
    /// * `content` - Message text displayed to the user.
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

    /// Builds an ephemeral acknowledgement for a command that will respond later.
    pub fn deferred_ephemeral() -> Self {
        Self {
            kind: InteractionCallbackType::DeferredChannelMessageWithSource,
            data: Some(InteractionCallbackData {
                content: None,
                flags: Some(MessageFlags::EPHEMERAL.bits()),
                choices: None,
            }),
        }
    }

    /// Builds an autocomplete response containing the supplied choices.
    ///
    /// # Arguments
    ///
    /// * `choices` - Values Discord presents for the focused command option.
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

/// Tests JSON payloads sent to Discord's interaction API.
#[cfg(test)]
mod tests {
    use super::{ApplicationCommandOptionChoice, InteractionResponse};

    /// Confirms that ordinary messages are marked ephemeral.
    #[test]
    fn serializes_ephemeral_message() {
        let response = InteractionResponse::ephemeral("Saved");

        assert_eq!(
            serde_json::to_value(response).expect("response should serialize"),
            serde_json::json!({
                "type": 4,
                "data": { "content": "Saved", "flags": 64 }
            })
        );
    }

    /// Confirms that command deferrals preserve ephemeral visibility.
    #[test]
    fn serializes_ephemeral_deferral() {
        let response = InteractionResponse::deferred_ephemeral();

        assert_eq!(
            serde_json::to_value(response).expect("response should serialize"),
            serde_json::json!({ "type": 5, "data": { "flags": 64 } })
        );
    }

    /// Confirms that autocomplete responses contain Discord-compatible choices.
    #[test]
    fn serializes_autocomplete_choices() {
        let response = InteractionResponse::autocomplete(vec![ApplicationCommandOptionChoice {
            name: "Moderator".to_string(),
            value: "Moderator".to_string(),
        }]);

        assert_eq!(
            serde_json::to_value(response).expect("response should serialize"),
            serde_json::json!({
                "type": 8,
                "data": { "choices": [{ "name": "Moderator", "value": "Moderator" }] }
            })
        );
    }
}
