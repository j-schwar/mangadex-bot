//! The `track` command tells the application to track a specific manga in the server
//! that the command was invoked in.

use std::sync::Arc;

use bson::{doc, Uuid};
use reqwest::Url;
use serenity::{
    async_trait,
    builder::CreateApplicationCommand,
    model::prelude::{
        command::CommandOptionType,
        interaction::{
            application_command::{ApplicationCommandInteraction, CommandDataOption},
            InteractionResponseType,
        },
    },
    prelude::Context,
};
use url::Host;

use crate::db::MongoClient;

use super::{CommandError, SlashCommand};

pub(super) struct Track {
    pub(super) db_client: Arc<MongoClient>,
}

#[async_trait]
impl SlashCommand for Track {
    fn build<'a>(
        &self,
        command: &'a mut CreateApplicationCommand,
    ) -> &'a mut CreateApplicationCommand {
        command
            .name("track")
            .description("Track updates for a given manga.")
            .create_option(|option| {
                option
                    .name("url")
                    .description("Manga URL or Id.")
                    .kind(CommandOptionType::String)
                    .required(true)
            })
    }

    async fn run(
        &self,
        ctx: Context,
        command: &ApplicationCommandInteraction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let say = |msg: String| async move {
            command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(msg))
                })
                .await
        };

        let options = command.data.options.as_slice();
        tracing::info!(
            command = command.data.name,
            ?options,
            "handling interaction"
        );

        // Extract the manga id from the command arguments.
        let manga_id = url_or_id(options)
            .and_then(manga_id_from_option)
            .ok_or_else(|| {
                tracing::error!(
                    command = command.data.name,
                    ?options,
                    "url or id option missing or invalid"
                );
                CommandError::ArgumentError
            })?
            .to_string();

        // Check if this manga already has a record in the database.
        let channel_id = command.channel_id;
        if let Some(mut manga) = self
            .db_client
            .read::<crate::db::Manga>(doc! { "_id": &manga_id })
            .await?
        {
            // If the manga is already tracked by this channel, then there's nothing left to do.
            if manga.channels.contains(&channel_id) {
                tracing::info!(?channel_id, %manga_id, "channel already tracks this manga");
                say(String::from(
                    "This manga is already tracked by this channel.",
                ))
                .await?;
                return Ok(());
            }

            // Otherwise, add this channel to the list of manga.
            let title = manga.title.clone();
            manga.channels.push(channel_id);
            self.db_client
                .update(doc! { "_id": &manga_id }, manga)
                .await?;

            // And send a response back to the user.
            say(format!("Now tracking {title}.")).await?;
        } else {
            // Otherwise, the manga does not already exist in the database so we need to insert it.
            let title = crate::mangadex::english_title(&manga_id)
                .await?
                .unwrap_or_else(|| manga_id.clone());

            let latest_chapter_id = crate::mangadex::latest_chapter(&manga_id)
                .await?
                .map(|c| c.id);

            let manga = crate::db::Manga {
                id: manga_id.clone(),
                title: title.clone(),
                latest_chapter_id,
                channels: vec![channel_id],
            };

            self.db_client.create(manga).await?;

            // And send a response back to the user.
            say(format!("Now tracking {title}.")).await?;
        }

        Ok(())
    }
}

/// Gets the url or id option from the list of options.
fn url_or_id(options: &[CommandDataOption]) -> Option<&str> {
    options
        .first()
        .and_then(|x| x.value.as_ref())
        .and_then(|x| x.as_str())
}

/// Extracts the manga id from a command options that is either an id or URL.
fn manga_id_from_option(url_or_id: &str) -> Option<Uuid> {
    if let Ok(id) = Uuid::parse_str(url_or_id) {
        Some(id)
    } else if let Ok(url) = Url::parse(url_or_id) {
        manga_id_from_url(url)
    } else {
        None
    }
}

/// Parses a Mangadex URL to a specific manga extracting the manga id.
fn manga_id_from_url(url: Url) -> Option<Uuid> {
    if Some(Host::Domain("mangadex.org")) != url.host() {
        return None;
    }

    let mut path_segments = url.path_segments()?;
    if "title" != path_segments.next()? {
        return None;
    }

    let id_str = path_segments.next()?;
    Uuid::parse_str(id_str).ok()
}
