use std::env;

use bson::{Bson, Document};
use mongodb::{options::ClientOptions, Client, Collection};
use serde::{Deserialize, Serialize};
use serenity::model::prelude::ChannelId;

/// Models a manga as it appears in the database.
#[derive(Serialize, Deserialize, Debug)]
pub struct Manga {
    /// The MangaDex id of the manga.
    #[serde(rename = "_id")]
    pub id: String,
    /// The english (or equivalent) title of the manga.
    pub title: String,
    /// The ids of the channels that are tracking this manga.
    pub channels: Vec<ChannelId>,
}

impl From<Manga> for Document {
    fn from(value: Manga) -> Self {
        let bson_value = bson::to_bson(&value).unwrap();
        let doc_value = bson_value.as_document().unwrap();
        doc_value.clone()
    }
}

/// Result type for database operations.
pub type Result<T> = std::result::Result<T, mongodb::error::Error>;

/// Client that connects to a MongoDB server.
pub struct MongoClient {
    collection: Collection<Document>,
    client: Client,
}

impl MongoClient {
    /// Connects to the mongo server using a given connection string.
    pub async fn connect(
        connection_string: &str,
        database: &str,
        collection: &str,
    ) -> Result<Self> {
        let options = ClientOptions::parse(connection_string).await?;
        let client = Client::with_options(options)?;
        let collection = client.database(database).collection(collection);

        Ok(Self { collection, client })
    }

    /// Connects to the mongo server, database, and collection specified by the following
    /// environment variables:
    ///
    /// * `MANGADEX_BOT_CONNECTION_STRING`
    /// * `MANGADEX_BOT_DATABASE`
    /// * `MANGADEX_BOT_COLLECTION`
    pub async fn from_env() -> Result<Self> {
        let connection_string = env::var("MANGADEX_BOT_CONNECTION_STRING").unwrap();
        let database = env::var("MANGADEX_BOT_DATABASE").unwrap();
        let collection = env::var("MANGADEX_BOT_COLLECTION").unwrap();

        Self::connect(&connection_string, &database, &collection).await
    }

    /// Creates a new document in the collection returning the id of the new document.
    pub async fn create(&self, doc: Document) -> Result<Bson> {
        self.collection
            .insert_one(doc, None)
            .await
            .map(|x| x.inserted_id)
    }

    /// Reads an existing document from the collection.
    pub async fn read(&self, doc: Document) -> Result<Option<Document>> {
        self.collection.find_one(Some(doc), None).await
    }

    /// Updates a document in the collection returning the number of records modified.
    pub async fn update(&self, filter: Document, update: Document) -> Result<u64> {
        self.collection
            .update_one(filter, update, None)
            .await
            .map(|x| x.modified_count)
    }

    /// Deletes a document from the collection returning the number of records deleted.
    pub async fn delete(&self, doc: Document) -> Result<u64> {
        self.collection
            .delete_one(doc, None)
            .await
            .map(|x| x.deleted_count)
    }
}
