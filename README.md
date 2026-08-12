# S-CyberSage

[![CI](https://github.com/JarredTD/S-CyberSage/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/JarredTD/S-CyberSage/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](LICENSE)

S-CyberSage is a serverless Discord bot for managing self-assignable roles in a single production guild. Administrators register roles, and members add or remove registered roles with slash commands.

The bot runs as an AWS Lambda behind API Gateway, stores role registrations in DynamoDB, and retrieves Discord credentials from Secrets Manager. Athenaeum provides the Discord interaction primitives; CyberSage defines the role commands and policies.

## Commands

| Command               | Who can use it | What it does                                                |
| --------------------- | -------------- | ----------------------------------------------------------- |
| `/role save <role>`   | Administrators | Registers a role as self-assignable.                        |
| `/role toggle <role>` | Any member     | Adds or removes a registered role from the invoking member. |

`/role save` also checks the bot's effective `Manage Roles` permission and Discord's role hierarchy before it stores a role. A role cannot be registered when the bot would be unable to manage it later.

## How it fits together

```text
Discord interaction
        |
API Gateway (prod)
        |
Lambda — verifies Discord's Ed25519 signature
        |
CyberSage role policy ── DynamoDB role registrations
        |
Discord REST API — member role changes
```

AWS CDK deploys two stacks:

- `S-CyberSageDataStack` creates the pay-per-request DynamoDB table and its role lookup indexes.
- `S-CyberSageControlStack` creates the HTTP API, ARM64 Lambda, log group, and two Secrets Manager secrets.

The configuration table is intentionally destroyed with the stack. It contains rebuildable role registrations, avoiding the cost of retaining an otherwise unused table after a teardown.

## Deploying

You need Node.js 24, Rust (as specified by [`s-cybersage-rs/rust-toolchain.toml`](s-cybersage-rs/rust-toolchain.toml)), Cargo Lambda, AWS credentials for the target account, and a Discord application.

```sh
npm ci
npm run check
npm run deploy
```

The first deployment creates empty Secrets Manager values. In the AWS console, set:

| Secret             | JSON key | Value                                                         |
| ------------------ | -------- | ------------------------------------------------------------- |
| Discord token      | `token`  | The bot token from the Discord Developer Portal.              |
| Discord public key | `key`    | The application public key from the Discord Developer Portal. |

CDK prints `ApiEndpoint` from the control stack. In the Discord Developer Portal, set that URL as the application's **Interactions Endpoint URL**.

The bot needs `Manage Roles`, and its highest role must sit above every role it should register or assign. Discord itself does not allow bots to manage roles at or above their own highest role.

## Registering commands

Copy the example environment file and fill in credentials for the target guild. Keep this file local: the token is a secret.

```sh
cp .env.example .env
npm run register-commands
```

`DISCORD_GUILD_ID` scopes command registration to one guild. Guild command updates are available immediately.

## License

S-CyberSage is licensed under the [GNU Affero General Public License v3.0](LICENSE).
