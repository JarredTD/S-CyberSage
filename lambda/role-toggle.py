import os
import json
import boto3
import requests
import logging
from nacl.signing import VerifyKey
from nacl.exceptions import BadSignatureError

# Set up logging
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

_discord_token = None
_discord_public_key = None


def get_discord_token():
    global _discord_token
    if _discord_token is None:
        secret = secrets_client.get_secret_value(SecretId=DISCORD_TOKEN_SECRET_ARN)
        _discord_token = json.loads(secret["SecretString"])["token"]
    return _discord_token


def get_discord_public_key():
    global _discord_public_key
    if _discord_public_key is None:
        secret = secrets_client.get_secret_value(SecretId=DISCORD_PUBLIC_KEY_SECRET_ARN)
        _discord_public_key = json.loads(secret["SecretString"])["key"]
    return _discord_public_key


def verify_discord_request(event):
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


def lookup_role_id(role_name: str) -> str | None:
    try:
        resp = table.get_item(Key={"roleName": role_name})
        return resp["Item"]["roleId"] if "Item" in resp else None
    except Exception as e:
        logger.error(f"DynamoDB lookup failed for role '{role_name}': {str(e)}")
        return None


def user_has_role(user_roles: list, role_id: str) -> bool:
    return role_id in user_roles


def add_role_to_user(guild_id: str, user_id: str, role_id: str, token: str):
    try:
        response = requests.put(
            f"{DISCORD_API_BASE}/guilds/{guild_id}/members/{user_id}/roles/{role_id}",
            headers={"Authorization": f"Bot {token}"},
            timeout=10,
        )
        response.raise_for_status()
        logger.info(f"Added role {role_id} to user {user_id} in guild {guild_id}")
        return True
    except requests.exceptions.RequestException as e:
        logger.error(f"Failed to add role {role_id} to user {user_id}: {str(e)}")
        return False


def remove_role_from_user(guild_id: str, user_id: str, role_id: str, token: str):
    try:
        response = requests.delete(
            f"{DISCORD_API_BASE}/guilds/{guild_id}/members/{user_id}/roles/{role_id}",
            headers={"Authorization": f"Bot {token}"},
            timeout=10,
        )
        response.raise_for_status()
        logger.info(f"Removed role {role_id} from user {user_id} in guild {guild_id}")
        return True
    except requests.exceptions.RequestException as e:
        logger.error(f"Failed to remove role {role_id} from user {user_id}: {str(e)}")
        return False


def handler(event, context):
    logger.info("Received Discord interaction request")

    if not verify_discord_request(event):
        logger.warning("Invalid request signature - rejecting")
        return {"statusCode": 401, "body": "Invalid request signature"}

    body = json.loads(event["body"])

    if body.get("type") == INTERACTION_TYPE_PING:
        logger.info("Handling PING interaction")
        return {"statusCode": 200, "body": json.dumps({"type": INTERACTION_TYPE_PING})}

    if body.get("type") == INTERACTION_TYPE_APPLICATION_COMMAND:
        command_name = body["data"]["name"]
        guild_id = body["guild_id"]
        user_id = body["member"]["user"]["id"]

        logger.info(
            f"Processing command '{command_name}' from user {user_id} in guild {guild_id}"
        )

        if command_name != "role":
            logger.warning(f"Unknown command received: {command_name}")
            return {
                "statusCode": 200,
                "body": json.dumps(
                    {
                        "type": INTERACTION_TYPE_APPLICATION_COMMAND,
                        "data": {
                            "content": "Unknown command",
                            "flags": INTERACTION_RESPONSE_EPHEMERAL,
                        },
                    }
                ),
            }

        role_name = body["data"]["options"][0]["value"]
        role_id = lookup_role_id(role_name)

        if not role_id:
            logger.warning(f"Role '{role_name}' not found in DynamoDB")
            return {
                "statusCode": 200,
                "body": json.dumps(
                    {
                        "type": INTERACTION_TYPE_APPLICATION_COMMAND,
                        "data": {
                            "content": f"Role '{role_name}' not found",
                            "flags": INTERACTION_RESPONSE_EPHEMERAL,
                        },
                    }
                ),
            }

        discord_token = get_discord_token()

        try:
            member_resp = requests.get(
                f"{DISCORD_API_BASE}/guilds/{guild_id}/members/{user_id}",
                headers={"Authorization": f"Bot {discord_token}"},
                timeout=10,
            )
            member_resp.raise_for_status()
            member = member_resp.json()

            if user_has_role(member["roles"], role_id):
                success = remove_role_from_user(
                    guild_id, user_id, role_id, discord_token
                )
                message = (
                    f"The '{role_name}' role was removed from you."
                    if success
                    else "Failed to remove role."
                )
                action = "remove"
            else:
                success = add_role_to_user(guild_id, user_id, role_id, discord_token)
                message = (
                    f"You were given the '{role_name}' role."
                    if success
                    else "Failed to add role."
                )
                action = "add"

            logger.info(
                f"Role {action} {'succeeded' if success else 'failed'} for user {user_id}, role {role_name}"
            )

            return {
                "statusCode": 200,
                "body": json.dumps(
                    {
                        "type": INTERACTION_TYPE_APPLICATION_COMMAND,
                        "data": {
                            "content": message,
                            "flags": INTERACTION_RESPONSE_EPHEMERAL,
                        },
                    }
                ),
            }

        except requests.exceptions.RequestException as e:
            logger.error(f"Discord API communication failed: {str(e)}")
            return {
                "statusCode": 200,
                "body": json.dumps(
                    {
                        "type": INTERACTION_TYPE_APPLICATION_COMMAND,
                        "data": {
                            "content": "Error communicating with Discord API",
                            "flags": INTERACTION_RESPONSE_EPHEMERAL,
                        },
                    }
                ),
            }

    logger.warning(f"Unknown interaction type received: {body.get('type')}")
    return {
        "statusCode": 400,
        "body": json.dumps({"error": "Unknown interaction type"}),
    }
