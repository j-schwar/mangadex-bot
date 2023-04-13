//! The `db` module contains types and functions for interacting with the database.
//!
//! This application uses an Azure Cosmos MongoDB NoSQL database to store the manga
//! tracked by various channels.

use std::sync::Arc;

use bson::{Bson, Document};
use mongodb::{options::ClientOptions, Client, Collection};
use serde::{Deserialize, Serialize};
use serenity::model::prelude::ChannelId;

/// Models a manga as it appears in the database.
#[derive(Serialize, Deserialize, Debug)]
pub struct Manga {
    /// The Id of the manga from MangaDex.
    #[serde(rename = "_id")]
    pub id: String,
    /// The english (or equivalent) title of the manga.
    pub title: String,
    /// The id of the latest chapter for this manga.
    pub latest_chapter_id: Option<String>,
    /// the ids of the channels that are tracking this manga.
    pub channels: Vec<ChannelId>,
}

impl From<Manga> for Document {
    fn from(value: Manga) -> Self {
        let value = bson::to_bson(&value).unwrap();
        let doc = value.as_document().unwrap();
        doc.clone()
    }
}

impl TryFrom<Document> for Manga {
    type Error = bson::de::Error;

    fn try_from(value: Document) -> std::result::Result<Self, Self::Error> {
        bson::from_bson(Bson::Document(value))
    }
}

/// Result type for database operations.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Client that connects to a MongoDB server.
#[derive(Debug)]
pub struct MongoClient {
    collection: Collection<Document>,
}

impl MongoClient {
    /// Connects to the mongo server using a given connection string.
    pub async fn connect(
        connection_string: &str,
        database: &str,
        collection: &str,
    ) -> Result<Arc<Self>> {
        let options = ClientOptions::parse(connection_string).await?;
        let client = Client::with_options(options)?;
        let collection = client.database(database).collection(collection);

        Ok(Arc::new(Self { collection }))
    }

    /// Creates a new document in the collection returning the id of the new document.
    #[tracing::instrument(err, skip_all)]
    pub async fn create<T>(&self, value: T) -> Result<Bson>
    where
        T: Into<Document>,
    {
        self.collection
            .insert_one(value.into(), None)
            .await
            .map(|x| x.inserted_id)
            .map_err(|err| err.into())
    }

    /// Reads an existing document from the collection.
    #[tracing::instrument(err, skip_all)]
    pub async fn read<T>(&self, doc: Document) -> Result<Option<T>>
    where
        T: TryFrom<Document>,
        T::Error: std::error::Error + Send + Sync + 'static,
    {
        let option = self.collection.find_one(Some(doc), None).await?;
        let option = match option {
            Some(doc) => T::try_from(doc).map(Some),
            None => Ok(None),
        }?;

        Ok(option)
    }

    /// Reads multiple documents from the collection.
    #[tracing::instrument(err, skip_all)]
    pub async fn read_many<'a, T>(&self, filter: Document) -> Result<Vec<T>>
    where
        T: TryFrom<Document>,
        T::Error: std::error::Error + Send + Sync + 'static,
    {
        let mut results = Vec::new();

        let mut cursor = self.collection.find(filter, None).await?;
        while cursor.advance().await? {
            let current = cursor.deserialize_current()?;
            let value = T::try_from(current)?;
            results.push(value);
        }

        Ok(results)
    }

    /// Updates a document in the collection returning the number of records modified.
    #[tracing::instrument(err, skip_all)]
    pub async fn update<T>(&self, filter: Document, update: T) -> Result<()>
    where
        T: Into<Document>,
    {
        self.collection
            .update_one(filter, update.into(), None)
            .await
            .map(|_| ())
            .map_err(|err| err.into())
    }

    /// Deletes a document from the collection returning the number of records deleted.
    #[allow(dead_code)]
    #[tracing::instrument(err, skip_all)]
    pub async fn delete(&self, doc: Document) -> Result<()> {
        self.collection
            .delete_one(doc, None)
            .await
            .map(|_| ())
            .map_err(|err| err.into())
    }
}
