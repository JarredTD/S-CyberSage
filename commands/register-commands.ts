import { config } from 'dotenv';

config();

import { REST } from '@discordjs/rest';
import { Routes } from 'discord-api-types/v10';

/**
 * Retrieves a required non-empty environment variable.
 *
 * @param name - Environment variable name.
 * @returns The configured variable value.
 */
function requireEnv(name: string): string {
  const value = process.env[name];

  if (value === undefined || value.trim() === '') {
    throw new Error(`Missing required environment variable: ${name}`);
  }

  return value;
}

const commands = [
  {
    name: 'role',
    description: 'Manage self-assignable roles',
    options: [
      {
        type: 1,
        name: 'toggle',
        description: 'Assign or remove a role',
        options: [
          {
            name: 'role',
            description: 'The role you want',
            type: 3,
            autocomplete: true,
            required: true,
          },
        ],
      },
      {
        type: 1,
        name: 'save',
        description: 'Register a role as self-assignable',
        default_member_permissions: '8',
        options: [
          {
            name: 'role',
            description: 'The role to register',
            type: 8,
            required: true,
          },
        ],
      },
    ],
  },
];

const rest = new REST({ version: '10' }).setToken(requireEnv('DISCORD_TOKEN'));

/** Registers the current command definitions for the configured Discord guild. */
async function registerCommands(): Promise<void> {
  try {
    console.log('Registering slash commands...');
    await rest.put(
      Routes.applicationGuildCommands(
        requireEnv('DISCORD_CLIENT_ID'),
        requireEnv('DISCORD_GUILD_ID'),
      ),
      { body: commands },
    );
    console.log('Commands registered.');
  } catch (err) {
    console.error(err);
    process.exitCode = 1;
  }
}

void registerCommands();
