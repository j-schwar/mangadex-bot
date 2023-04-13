//! The `scan` module contains functions check for new chapters.

use std::sync::Arc;
use std::time::Duration;

use bson::doc;
use serenity::http::Http;
use serenity::model::prelude::ChannelId;

use crate::db::{Manga, MongoClient};
use crate::mangadex::{self, Chapter, ChapterAttributes};

/// An endless task that periodically scans for chapter updates.
#[tracing::instrument(skip(http, db_client))]
pub async fn scan(http: Arc<Http>, db_client: Arc<MongoClient>, period: Duration) {
    loop {
        let _ = check_for_updates(&http, &db_client).await;
        tokio::time::sleep(period).await;
    }
}

/// For each manga in the database, queries MangaDex to see if any of them have new chapters.
#[tracing::instrument(err, skip_all)]
async fn check_for_updates(
    http: &Http,
    db_client: &MongoClient,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for manga in db_client.read_many::<Manga>(doc! {}).await? {
        if let Ok(Some(chapter)) = mangadex::latest_chapter(&manga.id).await {
            if Some(chapter.id.as_str()) != manga.latest_chapter_id.as_deref() {
                for channel in manga.channels.as_slice() {
                    // Ignore errors related to sending a message since there's not much we can do.
                    // TODO: One potential error may be that the channel does not exist. In that
                    //  case, we should remove the channel and all tracked manga.
                    let _ = send_update_message(http, &manga, &chapter, *channel).await;
                }

                let _ = db_client
                    .update(
                        doc! { "_id": &manga.id },
                        doc! { "$set": { "latest_chapter_id": &chapter.id } },
                    )
                    .await;
            }
        }

        // Add a bit of delay between each scan in order to avoid any rate limiting put
        // in place by MangaDex.
        // FIXME: A better solution would be to put rate limiting on the mangadex::latest_chapter function itself.
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Ok(())
}

/// Sends a message to a specific channel about a new chapter update.
#[tracing::instrument(err, skip(http))]
async fn send_update_message(
    http: &Http,
    manga: &Manga,
    chapter: &Chapter,
    channel: ChannelId,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manga_title = manga.title.as_str();
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

    channel.say(http, format!("{message}\n{url}")).await?;
    Ok(())
}
