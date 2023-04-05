//! The track command tells the application to track a given manga.
//!
//! Given the id of a manga or, more commonly the URL to the manga's Mangadex page, the
//! application adds said manga to the list of tracked manga for the channel that the
//! interaction as invoked on.

use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::CommandOptionType,
        interaction::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
        },
    },
    prelude::Context,
};
use url::{Host, Url};
use uuid::Uuid;

use crate::mangadex;
use crate::Handler;

/// Registers this command with the Discord client.
pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
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

/// Runs the handler for this command.
pub async fn run(handler: &Handler, ctx: Context, command: ApplicationCommandInteraction) {
    let options = command.data.options.as_slice();
    let url_or_id = options[0].value.as_ref().unwrap().as_str().unwrap();
    log::info!(
        "Received \"track\" interaction on channel {}: url or id = {url_or_id}",
        command.channel_id
    );

    if let Some(manga_id) = manga_id_from_option(url_or_id) {
        let title = match mangadex::english_title(manga_id).await {
            Ok(Some(title)) => title,
            Ok(None) => manga_id.to_string(),
            Err(err) => {
                println!("Error fetching manga title: {err}");
                return;
            }
        };

        // Update the application's tracking list in a critical section.
        {
            let mut app = handler.app.write().await;
            if let Err(_) = app.track(command.channel_id, manga_id).await {
                return;
            }
        }

        if let Err(err) = command
            .create_interaction_response(ctx.http, |response| {
                response
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| {
                        message.content(format!("Now tracking updates for {title}."))
                    })
            })
            .await
        {
            log::error!("Error constructing interaction response for \"track\": {err}");
        }
    } else {
        handle_invalid_url(ctx, command).await;
    }
}

/// Extracts the manga id from a command options that is either an id or URL.
fn manga_id_from_option(url_or_id: &str) -> Option<Uuid> {
    if let Ok(id) = Uuid::try_parse(url_or_id) {
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
    Uuid::try_parse(id_str).ok()
}

/// Responds to the interaction with an error message.
async fn handle_invalid_url(ctx: Context, command: ApplicationCommandInteraction) {
    if let Err(err) = command
        .create_interaction_response(ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message.content(format!("Please specify a valid manga id or url."))
                })
        })
        .await
    {
        println!("Error handling command \"track\": {err}");
    }
}
