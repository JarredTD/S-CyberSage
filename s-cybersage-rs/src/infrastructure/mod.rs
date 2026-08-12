/// Persists guild role registrations in DynamoDB.
pub mod dao;
/// Implements adapters for Discord's REST and interaction webhook APIs.
pub mod discord;
/// Retrieves secret values from AWS Secrets Manager.
pub mod reader;
