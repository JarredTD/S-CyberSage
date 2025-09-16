import { config } from "dotenv";
import path from "path";

config({ path: path.resolve(__dirname, "../.env") });

import { REST } from "@discordjs/rest";
import { Routes } from "discord-api-types/v10";

const commands = [
  {
    name: "role",
    description: "Assign or remove a role for yourself",
    options: [
      {
        name: "role_name",
        description: "The role you want",
        type: 3,
        required: true,
      },
    ],
  },
];

const rest = new REST({ version: "10" }).setToken(process.env.DISCORD_TOKEN!);

(async () => {
  try {
    console.log("Registering slash commands...");
    await rest.put(
      Routes.applicationGuildCommands(
        process.env.DISCORD_CLIENT_ID!,
        process.env.DISCORD_GUILD_ID!
      ),
      { body: commands }
    );
    console.log("Commands registered.");
  } catch (err) {
    console.error(err);
  }
})();
