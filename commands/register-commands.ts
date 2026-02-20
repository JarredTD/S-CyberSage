import { config } from "dotenv";

config();

import { REST } from "@discordjs/rest";
import { Routes } from "discord-api-types/v10";

const commands = [
  {
    name: "role",
    description: "Manage self-assignable roles",
    options: [
      {
        type: 1,
        name: "toggle",
        description: "Assign or remove a role",
        options: [
          {
            name: "role",
            description: "The role you want",
            type: 3,
            autocomplete: true,
            required: true,
          },
        ],
      },
      {
        type: 1,
        name: "save",
        description: "Register a role as self-assignable",
        default_member_permissions: "8",
        options: [
          {
            name: "role",
            description: "The role to register",
            type: 8,
            required: true,
          },
        ],
      },
    ],
  },
  {
    name: "subscribe",
    description: "Activate subscription for this guild",
    default_member_permissions: "8",
  },
  {
    name: "unsubscribe",
    description: "Deactivate subscription for this guild",
    default_member_permissions: "8",
  }
];

const rest = new REST({ version: "10" }).setToken(process.env.DISCORD_TOKEN!);

(async () => {
  try {
    console.log("Registering slash commands...");
    await rest.put(
      Routes.applicationGuildCommands(
        process.env.DISCORD_CLIENT_ID!,
        process.env.DISCORD_GUILD_ID!,
      ),
      { body: commands },
    );
    console.log("Commands registered.");
  } catch (err) {
    console.error(err);
  }
})();
