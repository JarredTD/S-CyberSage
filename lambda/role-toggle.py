import os
import json
import boto3
import requests
from nacl.signing import VerifyKey
from nacl.exceptions import BadSignatureError

DISCORD_API_BASE = "https://discord.com/api/v10"
INTERACTION_TYPE_CHANNEL_MESSAGE = 4
INTERACTION_RESPONSE_EPHEMERAL = 64

ROLE_TABLE = os.environ["ROLE_MAPPINGS_TABLE_NAME"]
DISCORD_TOKEN_SECRET_ARN = os.environ["DISCORD_TOKEN_SECRET_ARN"]
DISCORD_PUBLIC_KEY_SECRET_ARN = os.environ["DISCORD_PUBLIC_KEY_SECRET_ARN"]

dynamodb = boto3.resource("dynamodb")
table = dynamodb.Table(ROLE_TABLE)
secrets_client = boto3.client("secretsmanager")


def get_discord_token():
    secret = secrets_client.get_secret_value(SecretId=DISCORD_TOKEN_SECRET_ARN)
    return json.loads(secret["SecretString"])["token"]


def get_discord_public_key():
    secret = secrets_client.get_secret_value(SecretId=DISCORD_PUBLIC_KEY_SECRET_ARN)
    return json.loads(secret["SecretString"])["key"]


def verify_discord_request(event):
    """Verify incoming Discord interaction using Ed25519 signature"""
    try:
        signature = event["headers"]["x-signature-ed25519"]
        timestamp = event["headers"]["x-signature-timestamp"]
        body = event["body"]

        verify_key = VerifyKey(bytes.fromhex(get_discord_public_key()))
        verify_key.verify(f"{timestamp}{body}".encode(), bytes.fromhex(signature))
        return True
    except (KeyError, BadSignatureError):
        return False


def lookup_role_id(role_name: str) -> str | None:
    resp = table.get_item(Key={"roleName": role_name})
    return resp["Item"]["roleId"] if "Item" in resp else None


def user_has_role(user_roles: list, role_id: str) -> bool:
    return role_id in user_roles


def add_role_to_user(guild_id: str, user_id: str, role_id: str, token: str):
    requests.put(
        f"{DISCORD_API_BASE}/guilds/{guild_id}/members/{user_id}/roles/{role_id}",
        headers={"Authorization": f"Bot {token}"},
    )


def remove_role_from_user(guild_id: str, user_id: str, role_id: str, token: str):
    requests.delete(
        f"{DISCORD_API_BASE}/guilds/{guild_id}/members/{user_id}/roles/{role_id}",
        headers={"Authorization": f"Bot {token}"},
    )


def handler(event, context):
    if not verify_discord_request(event):
        return {"statusCode": 401, "body": "Invalid request signature"}

    body = json.loads(event["body"])
    command_name = body["data"]["name"]
    if command_name != "role":
        return {
            "statusCode": 200,
            "body": json.dumps(
                {
                    "type": INTERACTION_TYPE_CHANNEL_MESSAGE,
                    "data": {
                        "content": "Unknown command",
                        "flags": INTERACTION_RESPONSE_EPHEMERAL,
                    },
                }
            ),
        }

    role_name = body["data"]["options"][0]["value"]
    guild_id = body["guild_id"]
    user_id = body["member"]["user"]["id"]

    role_id = lookup_role_id(role_name)
    if not role_id:
        return {
            "statusCode": 200,
            "body": json.dumps(
                {
                    "type": INTERACTION_TYPE_CHANNEL_MESSAGE,
                    "data": {
                        "content": f"Role '{role_name}' not found",
                        "flags": INTERACTION_RESPONSE_EPHEMERAL,
                    },
                }
            ),
        }

    discord_token = get_discord_token()

    member_resp = requests.get(
        f"{DISCORD_API_BASE}/guilds/{guild_id}/members/{user_id}",
        headers={"Authorization": f"Bot {discord_token}"},
    )
    member_resp.raise_for_status()
    member = member_resp.json()

    if user_has_role(member["roles"], role_id):
        remove_role_from_user(guild_id, user_id, role_id, discord_token)
        message = f"The '{role_name}' role was removed from you."
    else:
        add_role_to_user(guild_id, user_id, role_id, discord_token)
        message = f"You were given the '{role_name}' role."

    return {
        "statusCode": 200,
        "body": json.dumps(
            {
                "type": INTERACTION_TYPE_CHANNEL_MESSAGE,
                "data": {"content": message, "flags": INTERACTION_RESPONSE_EPHEMERAL},
            }
        ),
    }
