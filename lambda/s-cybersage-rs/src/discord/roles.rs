pub async fn fetch_member_roles(
    client: &reqwest::Client,
    token: &str,
    guild_id: &str,
    user_id: &str,
) -> Result<Vec<String>, reqwest::Error> {
    let url = format!(
        "https://discord.com/api/v10/guilds/{}/members/{}",
        guild_id, user_id
    );
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await?
        .error_for_status()?;

    let json: serde_json::Value = resp.json().await?;
    Ok(json
        .get("roles")
        .and_then(|roles| roles.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default())
}

pub async fn modify_user_role(
    client: &reqwest::Client,
    token: &str,
    guild_id: &str,
    user_id: &str,
    role_id: &str,
    action: &str,
) -> bool {
    let url = format!(
        "https://discord.com/api/v10/guilds/{}/members/{}/roles/{}",
        guild_id, user_id, role_id
    );

    let request_builder = match action {
        "add" => client.put(&url),
        "remove" => client.delete(&url),
        _ => return false,
    };

    let resp = request_builder
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => true,
        Ok(r) if r.status().as_u16() == 403 => {
            tracing::error!("Bot permission error for {} role: {}", action, role_id);
            false
        }
        Ok(r) => {
            tracing::error!(
                "Failed to {} role {} for user {}: status {}",
                action,
                role_id,
                user_id,
                r.status()
            );
            false
        }
        Err(e) => {
            tracing::error!("Discord API request failed: {:?}", e);
            false
        }
    }
}
