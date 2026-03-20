# S(erverless)-CyberSage

S-CyberSage is a serverless adaptation of the CyberSage Discord bot, designed to manage self-assignable roles via slash commands.

## Functionality

### Commands

- `/role save <roleName>`
  - Saves a role to be toggleable
  - Requires administrator permissions
- `/role toggle <roleName>`
  - Toggles a role on the user
  - Available to all users

## Setup

### 1. Environment Variables

The following environment variables are required for command registration:

DISCORD_TOKEN=
DISCORD_CLIENT_ID=
DISCORD_GUILD_ID=

- **DISCORD_TOKEN** — Bot token  
- **DISCORD_CLIENT_ID** — Application (client) ID  
- **DISCORD_GUILD_ID** — Target guild for registering commands  

### 2. Secrets (AWS)

You must manually set the values for these secrets in the account you deployed to.

- Token
- Public Key

### 3. Discord Configuration

- Set the **Interactions Endpoint URL** in the Discord Developer Portal to your deployed API Gateway endpoint
- This allows Discord to send interaction events to the lambda

### 4. Command Registration

You must register slash commands before use.

- Use the provided registration script (or equivalent)
- Ensure the environment variables above are set before running
- Guild-level registration is recommended for faster updates during development

## Permissions

- `Manage Roles` — Required for the bot to add/remove roles from users  
- The bot’s highest role **must be above** any roles it is trying to assign/remove  

## License

This project is licensed under the AGPL-3.0 License. See the [LICENSE](LICENSE) file for details.