use std::collections::HashMap;

use serde::Deserialize;
use serde_repr::Deserialize_repr;

/// Identifies the kind of Discord interaction received by the webhook.
#[derive(Debug, Deserialize_repr, Clone, Copy)]
#[repr(u8)]
pub enum InteractionType {
    /// Initial verification request sent by Discord.
    Ping = 1,
    /// Invocation of an application command.
    ApplicationCommand = 2,
    /// Request for command-option autocomplete choices.
    ApplicationCommandAutocomplete = 4,

    /// An unsupported interaction type.
    #[serde(other)]
    Unknown,
}

/// Represents the subset of a Discord interaction request used by the handler.
#[derive(Debug, Deserialize, Clone)]
pub struct InteractionRequest {
    /// Snowflake identifier for this interaction.
    #[serde(default)]
    pub id: Option<String>,

    /// Snowflake identifier for the Discord application receiving the interaction.
    #[serde(default)]
    pub application_id: Option<String>,

    /// One-time interaction token used to send follow-up interaction responses.
    #[serde(default)]
    pub token: Option<String>,

    /// Type of interaction supplied by Discord.
    #[serde(rename = "type")]
    pub interaction_type: InteractionType,

    /// Command payload when the interaction is command-related.
    #[serde(default)]
    pub data: Option<ApplicationCommandData>,

    /// ID of the guild in which the interaction occurred.
    #[serde(default)]
    pub guild_id: Option<String>,

    /// Invoking member details for guild interactions.
    #[serde(default)]
    pub member: Option<Member>,
}

/// Contains application-command data sent by Discord.
#[derive(Debug, Deserialize, Clone)]
pub struct ApplicationCommandData {
    /// Top-level command name.
    pub name: String,

    /// Nested command options and subcommands.
    #[serde(default)]
    pub options: Vec<CommandOption>,

    /// Resolved role data included by Discord.
    #[serde(default)]
    pub resolved: Option<ResolvedData>,
}

/// Represents an individual Discord command option or subcommand.
#[derive(Debug, Deserialize, Clone)]
pub struct CommandOption {
    /// Option or subcommand name.
    pub name: String,

    /// User-supplied scalar value for this option.
    #[serde(default)]
    pub value: Option<serde_json::Value>,

    /// Nested options for a subcommand.
    #[serde(default)]
    pub options: Vec<CommandOption>,
}

/// Represents the member that invoked a guild interaction.
#[derive(Debug, Deserialize, Clone)]
pub struct Member {
    /// Guild permissions granted to the member as a decimal bitfield string.
    #[serde(default)]
    pub permissions: Option<String>,

    /// Discord user associated with the member.
    pub user: User,
}

/// Represents the invoking Discord user.
#[derive(Debug, Deserialize, Clone)]
pub struct User {
    /// Discord snowflake identifier for the user.
    pub id: String,
}

/// Contains entities resolved by Discord for a command interaction.
#[derive(Debug, Deserialize, Clone)]
pub struct ResolvedData {
    /// Resolved roles keyed by Discord role ID.
    #[serde(default)]
    pub roles: HashMap<String, ResolvedRole>,
}

/// Represents a role resolved by Discord for a command.
#[derive(Debug, Deserialize, Clone)]
pub struct ResolvedRole {
    /// Display name of the role.
    pub name: String,
}

/// Tests deserialization of Discord's interaction payloads from representative fixtures.
#[cfg(test)]
mod tests {
    use super::{InteractionRequest, InteractionType};

    /// Confirms that a guild command retains callback, permission, option, and role data.
    #[test]
    fn deserializes_guild_command() {
        let interaction: InteractionRequest = serde_json::from_value(serde_json::json!({
            "id": "interaction-id",
            "application_id": "application-id",
            "token": "interaction-token",
            "type": 2,
            "guild_id": "guild-id",
            "member": { "permissions": "8", "user": { "id": "user-id" } },
            "data": {
                "name": "role",
                "options": [{
                    "name": "save",
                    "options": [{ "name": "role", "value": "role-id" }]
                }],
                "resolved": { "roles": { "role-id": { "name": "Moderator" } } }
            }
        }))
        .expect("fixture should match Discord's interaction schema");

        assert!(matches!(
            interaction.interaction_type,
            InteractionType::ApplicationCommand
        ));
        assert_eq!(
            interaction
                .member
                .as_ref()
                .and_then(|member| member.permissions.as_deref()),
            Some("8")
        );
        assert_eq!(
            interaction
                .data
                .as_ref()
                .and_then(|data| data.resolved.as_ref())
                .and_then(|resolved| resolved.roles.get("role-id"))
                .map(|role| role.name.as_str()),
            Some("Moderator")
        );
    }

    /// Confirms that unsupported Discord interaction types deserialize safely.
    #[test]
    fn deserializes_unknown_interaction_type() {
        let interaction: InteractionRequest = serde_json::from_str(r#"{ "type": 99 }"#)
            .expect("unknown interaction type should deserialize");

        assert!(matches!(
            interaction.interaction_type,
            InteractionType::Unknown
        ));
    }
}
