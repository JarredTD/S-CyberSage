# S-CyberSage

[![CI](https://github.com/JarredTD/S-CyberSage/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/JarredTD/S-CyberSage/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](LICENSE)

S-CyberSage is a serverless Discord bot for managing self-assignable roles in a guild. Administrators register roles, and members add or remove registered roles with slash commands.

## Features

- Registers roles that guild members may assign to themselves.
- Enforces Discord permissions and role hierarchy before accepting a role.
- Stores role registrations in DynamoDB.
- Verifies Discord interaction signatures before processing commands.
- Deploys as an AWS Lambda and HTTP API through AWS CDK.

## Setup

Install Node.js 24, Rust from `rust-toolchain.toml`, Cargo Lambda, and Zig 0.14. Then install the locked Node dependencies:

```sh
npm ci
```

AWS deployment requires credentials for the target account and an existing Discord application.

## Usage

| Command               | Who can use it | Result                                      |
| --------------------- | -------------- | ------------------------------------------- |
| `/role save <role>`   | Administrators | Registers a role as self-assignable.        |
| `/role toggle <role>` | Any member     | Adds or removes a registered role.          |

To deploy the application and register its guild commands:

```sh
npm run deploy
npm run register-commands
```

## Configuration

Copy `.env.example` to `.env` and set the Discord application, bot, and guild values used by command registration. Keep `.env` local because it contains a bot token.

The first deployment creates empty Secrets Manager values. Store the Discord bot token under `token` and the application public key under `key`. Set the deployed `ApiEndpoint` output as the Discord application's Interactions Endpoint URL.

The bot requires `Manage Roles`, and its highest role must be above every role it can register or assign.

## Architecture

Discord sends interactions through API Gateway to a Rust Lambda. The Lambda verifies each Ed25519 signature, applies CyberSage's role policy, persists registrations in DynamoDB, and calls the Discord REST API for member role changes.

AWS CDK owns two stacks:

- `S-CyberSageDataStack` creates the pay-per-request DynamoDB table and role lookup indexes.
- `S-CyberSageControlStack` creates the HTTP API, ARM64 Lambda, log group, and Secrets Manager secrets.

The table uses a destroy removal policy because role registrations are rebuildable configuration.

## License

S-CyberSage is licensed under the [GNU Affero General Public License v3.0 only](LICENSE).
