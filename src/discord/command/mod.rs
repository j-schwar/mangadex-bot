//! The `command` module contains the core trait for slash (aka. application) command
//! handlers.

use std::{collections::HashMap, fmt::Display, sync::Arc};

use serenity::{
    async_trait, builder::CreateApplicationCommand,
    model::prelude::interaction::application_command::ApplicationCommandInteraction,
    prelude::Context,
};

use crate::db::MongoClient;

mod track;

/// Error type returned by slash command handlers.
#[derive(Debug, Clone, Copy)]
pub enum CommandError {
    ArgumentError,
}

impl Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::ArgumentError => f.write_str("argument error"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Core trait that all slash commands implement.
#[async_trait]
pub trait SlashCommand: Send + Sync {
    /// Builds the metadata for this command using a provided builder.
    fn build<'a>(
        &self,
        command: &'a mut CreateApplicationCommand,
    ) -> &'a mut CreateApplicationCommand;

    /// The handler for the command.
    async fn run(
        &self,
        ctx: Context,
        command: &ApplicationCommandInteraction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// A mapping of command names to their implementations.
pub type SlashCommandMap = HashMap<String, Box<dyn SlashCommand>>;

/// Initializes the set of slash commands for this bot.
#[tracing::instrument]
pub(crate) fn init(args: &crate::Args, db_client: Arc<MongoClient>) -> SlashCommandMap {
    let mut commands: SlashCommandMap = HashMap::new();

    commands.insert(
        String::from("track"),
        Box::new(track::Track {
            db_client,
        }),
    );

    commands
}
