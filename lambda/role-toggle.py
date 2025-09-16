import os
import json
import logging
from typing import Optional, List

import boto3
import requests
from nacl.signing import VerifyKey
from nacl.exceptions import BadSignatureError

logger = logging.getLogger()
logger.setLevel(logging.INFO)

DISCORD_API_BASE = "https://discord.com/api/v10"
INTERACTION_TYPE_PING = 1
INTERACTION_TYPE_APPLICATION_COMMAND = 4
INTERACTION_RESPONSE_EPHEMERAL = 64

ROLE_TABLE = os.environ["ROLE_MAPPINGS_TABLE_NAME"]
DISCORD_TOKEN_SECRET_ARN = os.environ["DISCORD_TOKEN_SECRET_ARN"]
DISCORD_PUBLIC_KEY_SECRET_ARN = os.environ["DISCORD_PUBLIC_KEY_SECRET_ARN"]

dynamodb = boto3.resource("dynamodb")
table = dynamodb.Table(ROLE_TABLE)
secrets_client = boto3.client("secretsmanager")

_discord_token: Optional[str] = None
_discord_public_key: Optional[str] = None


def get_discord_token() -> str:
    global _discord_token
    if _discord_token is None:
        secret = secrets_client.get_secret_value(SecretId=DISCORD_TOKEN_SECRET_ARN)
        _discord_token = json.loads(secret["SecretString"])["token"]
    assert _discord_token is not None
    return _discord_token


def get_discord_public_key() -> str:
    global _discord_public_key
    if _discord_public_key is None:
        secret = secrets_client.get_secret_value(SecretId=DISCORD_PUBLIC_KEY_SECRET_ARN)
        _discord_public_key = json.loads(secret["SecretString"])["key"]
    assert _discord_public_key is not None
    return _discord_public_key


def verify_discord_request(event) -> bool:
    """Verify incoming Discord interaction using Ed25519 signature"""
    try:
        signature = event["headers"]["x-signature-ed25519"]
        timestamp = event["headers"]["x-signature-timestamp"]
        body = event["body"]

        verify_key = VerifyKey(bytes.fromhex(get_discord_public_key()))
        verify_key.verify(f"{timestamp}{body}".encode(), bytes.fromhex(signature))
        return True
    except (KeyError, BadSignatureError, ValueError) as e:
        logger.warning(f"Signature verification failed: {str(e)}")
        return False


def lookup_role_id(role_name: str) -> Optional[str]:
    try:
        resp = table.get_item(Key={"roleName": role_name})
        return resp["Item"]["roleId"] if "Item" in resp else None
    except Exception as e:
        logger.error(f"DynamoDB lookup failed for role '{role_name}': {str(e)}")
        return None


def user_has_role(user_roles: List[str], role_id: str) -> bool:
    return role_id in user_roles


def modify_user_role(guild_id: str, user_id: str, role_id: str, action: str) -> bool:
    """Add or remove a role. action='add' or 'remove'"""
    discord_token = get_discord_token()
    method = requests.put if action == "add" else requests.delete
    url = f"{DISCORD_API_BASE}/guilds/{guild_id}/members/{user_id}/roles/{role_id}"

    try:
        resp = method(
            url, headers={"Authorization": f"Bot {discord_token}"}, timeout=10
        )
        resp.raise_for_status()
        logger.info(
            f"{action.title()}ed role {role_id} for user {user_id} in guild {guild_id}"
        )
        return True
    except requests.exceptions.HTTPError as e:
        status_code = e.response.status_code if e.response else "unknown"
        if status_code == 403:
            logger.error(f"Bot permission error for {action} role: {role_id}")
        else:
            logger.error(
                f"Failed to {action} role {role_id} for user {user_id}: {str(e)}"
            )
        return False
    except requests.exceptions.RequestException as e:
        logger.error(f"Discord API request failed: {str(e)}")
        return False


def get_member_roles(guild_id: str, user_id: str) -> Optional[List[str]]:
    """Retrieve user's current roles in a guild"""
    try:
        discord_token = get_discord_token()
        resp = requests.get(
            f"{DISCORD_API_BASE}/guilds/{guild_id}/members/{user_id}",
            headers={"Authorization": f"Bot {discord_token}"},
            timeout=10,
        )
        resp.raise_for_status()
        return resp.json().get("roles", [])
    except requests.exceptions.RequestException as e:
        logger.error(f"Failed to fetch member roles: {str(e)}")
        return None


def handler(event, context):
    logger.info("Received Discord interaction request")

    if not verify_discord_request(event):
        return {"statusCode": 401, "body": "Invalid request signature"}

    body = json.loads(event["body"])
    interaction_type = body.get("type")

    if interaction_type == INTERACTION_TYPE_PING:
        return {"statusCode": 200, "body": json.dumps({"type": INTERACTION_TYPE_PING})}

    if interaction_type == INTERACTION_TYPE_APPLICATION_COMMAND:
        command_name = body.get("data", {}).get("name")
        options = body.get("data", {}).get("options", [])
        guild_id = body.get("guild_id")
        user_id = body.get("member", {}).get("user", {}).get("id")

        if command_name != "role":
            return {
                "statusCode": 200,
                "body": json.dumps(
                    {
                        "type": 4,
                        "data": {
                            "content": "Unknown command",
                            "flags": INTERACTION_RESPONSE_EPHEMERAL,
                        },
                    }
                ),
            }

        if not options or "value" not in options[0]:
            return {
                "statusCode": 200,
                "body": json.dumps(
                    {
                        "type": 4,
                        "data": {
                            "content": "Role name is missing",
                            "flags": INTERACTION_RESPONSE_EPHEMERAL,
                        },
                    }
                ),
            }

        role_name = options[0]["value"]
        role_id = lookup_role_id(role_name)

        if not role_id:
            return {
                "statusCode": 200,
                "body": json.dumps(
                    {
                        "type": 4,
                        "data": {
                            "content": f"Role '{role_name}' not found",
                            "flags": INTERACTION_RESPONSE_EPHEMERAL,
                        },
                    }
                ),
            }

        member_roles = get_member_roles(guild_id, user_id)
        if member_roles is None:
            return {
                "statusCode": 200,
                "body": json.dumps(
                    {
                        "type": 4,
                        "data": {
                            "content": "Failed to fetch your roles",
                            "flags": INTERACTION_RESPONSE_EPHEMERAL,
                        },
                    }
                ),
            }

        if user_has_role(member_roles, role_id):
            success = modify_user_role(guild_id, user_id, role_id, "remove")
            message = (
                f"The '{role_name}' role was removed from you."
                if success
                else "Failed to remove role."
            )
        else:
            success = modify_user_role(guild_id, user_id, role_id, "add")
            message = (
                f"You were given the '{role_name}' role."
                if success
                else "Failed to add role."
            )

        return {
            "statusCode": 200,
            "body": json.dumps(
                {
                    "type": 4,
                    "data": {
                        "content": message,
                        "flags": INTERACTION_RESPONSE_EPHEMERAL,
                    },
                }
            ),
        }

    logger.warning(f"Unknown interaction type: {interaction_type}")
    return {
        "statusCode": 400,
        "body": json.dumps({"error": "Unknown interaction type"}),
    }
