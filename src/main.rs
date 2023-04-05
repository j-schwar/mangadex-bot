use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::Arc;
use std::time::Duration;

use mangadex::{Chapter, ChapterAttributes};
use serenity::async_trait;
use serenity::http::Http;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::model::prelude::{ChannelId, GuildId};
use serenity::prelude::*;
use tokio::sync::mpsc;
use uuid::Uuid;

mod commands;
mod mangadex;

/// The id of a manga.
pub type MangaId = Uuid;

/// The id of a chapter.
pub type ChapterId = Uuid;

/// The id of the latest chapter for a manga.
pub type LatestChapterId = Option<ChapterId>;

pub struct App {
    pub data: HashMap<MangaId, (LatestChapterId, HashSet<ChannelId>)>,
}

impl App {
    fn singleton() -> Arc<RwLock<App>> {
        let app = App {
            data: HashMap::new(),
        };

        Arc::new(RwLock::new(app))
    }

    /// Tracks a specific manga for a given channel.
    ///
    /// The latest existing chapter for this manga is fetched at this time and stored alongside
    /// the manga id to be referenced later when searching for updates.
    async fn track(&mut self, channel_id: ChannelId, manga_id: MangaId) -> mangadex::Result<()> {
        let latest_chapter_id = mangadex::latest_chapter(manga_id)
            .await?
            .map(|c| Uuid::try_parse(&c.id).unwrap());
        log::debug!("Found latest chapter for {manga_id} to be {latest_chapter_id:?}");

        match self.data.get_mut(&manga_id) {
            Some((existing_chapter_id, channels)) => {
                *existing_chapter_id = latest_chapter_id;
                channels.insert(channel_id);
            }

            None => {
                let mut channels = HashSet::with_capacity(1);
                channels.insert(channel_id);

                self.data.insert(manga_id, (latest_chapter_id, channels));
            }
        }

        log::info!("Tracking manga {manga_id} on channel {channel_id}");
        Ok(())
    }

    /// Sets the latest chapter id for a given manga.
    fn set_latest_chapter_id(&mut self, manga_id: MangaId, chapter_id: ChapterId) {
        if let Some((existing_chapter_id, _)) = self.data.get_mut(&manga_id) {
            *existing_chapter_id = Some(chapter_id);
        }
    }
}

pub struct Handler {
    pub app: Arc<RwLock<App>>,
}

impl From<Arc<RwLock<App>>> for Handler {
    fn from(value: Arc<RwLock<App>>) -> Self {
        Handler { app: value }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            match command.data.name.as_str() {
                "track" => commands::track::run(&self, ctx, command).await,

                "ping" => {
                    if let Err(why) = command
                        .create_interaction_response(&ctx.http, |response| {
                            let message = command.data.options[0]
                                .value
                                .as_ref()
                                .unwrap()
                                .as_str()
                                .unwrap();
                            let response_message = format!("Pong: {}", message);

                            response
                                .kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|message| {
                                    message.content(response_message)
                                })
                        })
                        .await
                    {
                        println!("Cannot respond to slash command: {}", why);
                    }
                }

                _ => {}
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        log::info!("{} is now connected!", ready.user.name);

        let guild_id = GuildId(1091577155672084490);
        guild_id
            .set_application_commands(&ctx.http, |commands| {
                commands.create_application_command(|command| commands::track::register(command))
            })
            .await
            .unwrap();

        //
        // Discord setup is now finished, spawn long running tasks.
        //

        let (tx, rx) = mpsc::channel(100);
        let app = self.app.clone();
        tokio::spawn(async move {
            periodically_scan_for_updates(app, tx).await;
        });

        let app = self.app.clone();
        tokio::spawn(async move {
            listen_for_fetch_events(ctx.http.clone(), app, rx).await;
        });
    }
}

async fn periodically_scan_for_updates(
    app: Arc<RwLock<App>>,
    tx: mpsc::Sender<(MangaId, LatestChapterId, Vec<ChannelId>)>,
) {
    loop {
        let data = {
            let app = app.read().await;
            app.data.clone()
        };

        for (manga_id, (latest_chapter_id, channels)) in data {
            if let Err(err) = tx
                .send((manga_id, latest_chapter_id, channels.into_iter().collect()))
                .await
            {
                log::error!("Event receiver dropped: {err}");
                break;
            }

            // We don't want to exceed any rate limits on the MangaDex API with our
            // requests so we'll delay a bit before fetching checking each manga.
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

/// Listens on a channel for manga to check for updates.
///
/// Another task should periodically send manga across this channel for this task to
/// check. This separation of logic helps reduce the complexity of the application. If a
/// new chapter is found the database in [App] is updated with the new chapter and a
/// message is sent to the corresponding channels using [send_new_chapter_message].
async fn listen_for_fetch_events(
    http: Arc<Http>,
    app: Arc<RwLock<App>>,
    mut rx: mpsc::Receiver<(MangaId, LatestChapterId, Vec<ChannelId>)>,
) {
    while let Some((manga_id, latest_chapter_id, channels)) = rx.recv().await {
        log::debug!(
            "Searching for new chapter for manga = {manga_id} as requested by {} channels, previously known latest chapter was {latest_chapter_id:?}",
            channels.len()
        );

        if let Ok(Some(chapter)) = mangadex::updated_chapter(manga_id, latest_chapter_id).await {
            log::info!("Found new chapter = {} for manga {manga_id}", chapter.id);

            {
                let mut app = app.write().await;
                let chapter_id = Uuid::try_parse(&chapter.id).unwrap();
                app.set_latest_chapter_id(manga_id, chapter_id);
            }

            for channel_id in channels {
                let _ =
                    send_new_chapter_message(http.clone(), channel_id, manga_id, &chapter).await;
            }
        } else {
            log::debug!("Did not find any new chapters for manga = {manga_id}");
        }
    }
}

/// Sends a message to a channel about a new chapter for a specific manga.
async fn send_new_chapter_message(
    http: Arc<Http>,
    channel_id: ChannelId,
    manga_id: MangaId,
    chapter: &Chapter,
) -> mangadex::Result<()> {
    let manga_title = mangadex::english_title(manga_id)
        .await?
        .unwrap_or_else(|| manga_id.to_string());

    let url = chapter.url();
    let message = match &chapter.attributes {
        ChapterAttributes {
            chapter: Some(ch),
            title: Some(title),
            ..
        } => format!("New chapter!\n{manga_title} ch. {ch}: {title}"),
        ChapterAttributes {
            chapter: Some(ch), ..
        } => format!("New chapter!\n{manga_title} ch. {ch}"),
        _ => format!("New chapter for {manga_title}!"),
    };

    if let Err(err) = channel_id.say(http, format!("{message}\n{url}")).await {
        log::error!("Error sending message to channel {channel_id}: {err}");
        // Ignore errors related to sending a message since there's not much we can do.
        // TODO: One potential error may be that the channel does not exist.
        //  In that case, we should remove the channel and all tracked manga.
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let app = App::singleton();

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler::from(app.clone()))
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        log::error!("An error occurred while running the client: {why}");
    }
}
