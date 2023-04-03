use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use serenity::async_trait;
use serenity::http::Http;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::model::prelude::{ChannelId, GuildId};
use serenity::prelude::*;
use uuid::Uuid;

mod commands;
mod mangadex;

pub struct App {
    pub http: Option<Arc<Http>>,
    pub data: HashMap<ChannelId, Vec<Uuid>>,
}

impl App {
    fn singleton() -> Arc<RwLock<App>> {
        let app = App {
            http: None,
            data: HashMap::new(),
        };

        Arc::new(RwLock::new(app))
    }

    fn set_http(&mut self, http: Arc<Http>) {
        self.http = Some(http);
    }

    fn track(&mut self, channel_id: ChannelId, manga_id: Uuid) {
        match self.data.get_mut(&channel_id) {
            Some(manga_ids) => {
                manga_ids.push(manga_id);
            }

            None => {
                self.data.insert(channel_id, vec![manga_id]);
            }
        }

        println!("Tracking manga {manga_id} on channel {channel_id}");
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
        println!("{} is now connected!", ready.user.name);

        // Client is now ready, copy the HTTP delegate to the application so it can be
        // used elsewhere within the program.
        {
            let mut app = self.app.write().await;
            app.set_http(ctx.http.clone());
        }

        let guild_id = GuildId(1091577155672084490);
        guild_id
            .set_application_commands(&ctx.http, |commands| {
                commands.create_application_command(|command| commands::track::register(command))
            })
            .await
            .unwrap();
    }
}

async fn periodically_scan_for_updates(app: Arc<RwLock<App>>) {
    loop {
        // FIXME: We don't want to hold this reader lock while we're querying Mangadex.
        {
            let app = app.read().await;
            if let Some(_http) = app.http.clone() {
                println!("Scanning for updates now...");
                // TODO: implement me
            }
        }

        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

#[tokio::main]
async fn main() {
    let app = App::singleton();

    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("token");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(Handler::from(app.clone()))
        .await
        .expect("Error creating client");

    tokio::spawn(async move {
        periodically_scan_for_updates(app.clone()).await;
    });

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}
