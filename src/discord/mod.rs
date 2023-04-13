use std::{sync::Arc, time::Duration};

use serenity::{
    async_trait,
    http::Http,
    model::{
        application::interaction::Interaction,
        prelude::{GuildId, Ready},
    },
    prelude::*,
    Client,
};

use crate::db::MongoClient;

use self::command::SlashCommandMap;

pub mod command;

/// Implementation of [EventHandler] for handling discord events.
struct Handler {
    guild_id: Option<u64>,
    scan_period: u64,
    db_client: Arc<MongoClient>,
    commands: SlashCommandMap,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!(
            name = ready.user.name,
            "MangaDex discord bot is now connected!"
        );

        // Setup application commands for this bot.
        init_application_commands(&ctx.http, self.guild_id, &self.commands)
            .await
            .expect("failed to initialize application commands");

        // Spawn background tasks to scan for updates from MangaDex.
        let http = ctx.http.clone();
        let db_client = self.db_client.clone();
        let period = Duration::from_secs(self.scan_period);
        tokio::spawn(async move {
            crate::scan::scan(http, db_client, period).await;
        });
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            // Find the command handler from the list of registered commands.
            if let Some(handler) = self.commands.get(&command.data.name) {
                // Invoke the handler. Logging any error that occurs.
                //
                // It's also possible to response to the command with an error message here,
                // however for now we'll just let the command timeout and discord will present an
                // error message for us.
                if let Err(err) = handler.run(ctx, &command).await {
                    tracing::error!(%err, ?command, name = command.data.name, "error handling application command");
                }
            } else {
                tracing::warn!(?command, "unknown command");
            }
        }
    }
}

/// Initializes and returns the discord client.
pub async fn init(
    token: &str,
    guild_id: Option<u64>,
    scan_period: u64,
    db_client: Arc<MongoClient>,
    commands: SlashCommandMap,
) -> serenity::Result<Client> {
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    let handler = Handler {
        guild_id,
        scan_period,
        db_client,
        commands,
    };
    Client::builder(token, intents).event_handler(handler).await
}

async fn init_application_commands(
    http: &Http,
    guild_id: Option<u64>,
    commands: &SlashCommandMap,
) -> serenity::Result<()> {
    if let Some(guild_id) = guild_id {
        GuildId(guild_id)
            .set_application_commands(http, |mut builder| {
                for command in commands.values() {
                    builder = builder.create_application_command(|builder| command.build(builder))
                }

                builder
            })
            .await
            .map_err(|err| {
                tracing::error!(%err, %guild_id, "failed to initialize guild specific application commands");
                err
            })?;
    }

    Ok(())
}
