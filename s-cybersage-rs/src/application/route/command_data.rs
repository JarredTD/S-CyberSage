use std::collections::HashMap;

use serde::Deserialize;

/// Represents the command payload understood by CyberSage's role bot.
#[derive(Debug, Deserialize, Clone)]
pub struct ApplicationCommandData {
    /// Top-level command name.
    pub name: String,
    /// Nested command options and subcommands.
    #[serde(default)]
    pub options: Vec<CommandOption>,
    /// Role entities resolved by Discord for the command.
    #[serde(default)]
    pub resolved: Option<ResolvedData>,
}

/// Represents an individual CyberSage command option or subcommand.
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

/// Contains entities resolved by Discord for a CyberSage command interaction.
#[derive(Debug, Deserialize, Clone)]
pub struct ResolvedData {
    /// Roles keyed by Discord role ID.
    #[serde(default)]
    pub roles: HashMap<String, ResolvedRole>,
}

/// Represents a role resolved by Discord for a CyberSage command.
#[derive(Debug, Deserialize, Clone)]
pub struct ResolvedRole {
    /// Display name of the resolved role.
    pub name: String,
}
