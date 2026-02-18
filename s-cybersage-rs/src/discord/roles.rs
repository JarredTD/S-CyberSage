use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tracing;

#[derive(Debug, Clone, Copy)]
pub enum RoleAction {
    Add,
    Remove,
}

#[derive(Debug, Deserialize)]
struct GuildMember {
    roles: Vec<String>,
}

pub async fn fetch_member_roles(
    client: &Client,
    token: &str,
    guild_id: &str,
    user_id: &str,
) -> Result<Vec<String>> {
    let url = format!(
        "https://discord.com/api/v10/guilds/{}/members/{}",
        guild_id, user_id
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await
        .context("Failed to send fetch_member_roles request")?
        .error_for_status()
        .context("Discord returned error while fetching member")?;

    let member: GuildMember = resp
        .json()
        .await
        .context("Failed to deserialize GuildMember")?;

    Ok(member.roles)
}

pub async fn modify_user_role(
    client: &Client,
    token: &str,
    guild_id: &str,
    user_id: &str,
    role_id: &str,
    action: RoleAction,
) -> Result<()> {
    let url = format!(
        "https://discord.com/api/v10/guilds/{}/members/{}/roles/{}",
        guild_id, user_id, role_id
    );

    let request_builder = match action {
        RoleAction::Add => client.put(&url),
        RoleAction::Remove => client.delete(&url),
    };

    let resp = request_builder
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await
        .context("Failed to send modify_user_role request")?;

    match resp.status() {
        s if s.is_success() => {
            tracing::info!(
                "Successfully {:?} role {} for user {}",
                action,
                role_id,
                user_id
            );
            Ok(())
        }

        StatusCode::FORBIDDEN => {
            tracing::error!(
                "Permission error while {:?} role {} for user {}",
                action,
                role_id,
                user_id
            );
            anyhow::bail!("Bot lacks permission to modify role")
        }

        StatusCode::TOO_MANY_REQUESTS => {
            tracing::warn!(
                "Rate limited while {:?} role {} for user {}",
                action,
                role_id,
                user_id
            );
            anyhow::bail!("Rate limited by Discord API")
        }

        other => {
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(
                "Failed to {:?} role {} for user {}: status {}, body: {}",
                action,
                role_id,
                user_id,
                other,
                body
            );
            anyhow::bail!("Discord API error: {}", other);
        }
    }
}
