use std::{collections::HashMap, fmt::Display};

use crate::{LatestChapterId, MangaId};
use serde::Deserialize;
use url::Url;
use uuid::Uuid;

const SITE: &'static str = "https://api.mangadex.org";

/// An error returned by the MangaDex API.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    id: String,
    status: i32,
    title: String,
    detail: String,
}

/// Errors returned by Mangadex operations.
#[derive(Debug, Clone)]
pub enum Error {
    NetworkError,
    Api(Vec<ApiError>),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match &self {
            NetworkError => f.write_str("An error occurred while communicating with the MangaDex."),
            Api(errors) => match errors.len() {
                0 => f.write_str("An error was returned by the MangaDex API."),
                1 => write!(
                    f,
                    "An error was returned by the MangaDex API: {}",
                    errors[0].detail
                ),
                _ => f.write_str(
                    "Many errors were returned by the MangaDex API, see logs for more information.",
                ),
            },
        }
    }
}

impl std::error::Error for Error {}

/// Result type returned by Mangadex operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Models a response from the MangaDex API that contains a single entity.
#[derive(Debug, Deserialize)]
#[serde(tag = "result")]
#[serde(rename_all = "camelCase")]
enum EntityResponse<T> {
    Ok { data: T },
    Error { errors: Vec<ApiError> },
}

impl<T> EntityResponse<T> {
    /// Converts this response into a [Result].
    fn into_result(self) -> Result<T> {
        match self {
            EntityResponse::Ok { data } => Ok(data),
            EntityResponse::Error { errors } => Err(Error::Api(errors)),
        }
    }
}

/// Models a response from the MangaDex API that contains multiple entities.
#[derive(Debug, Deserialize)]
#[serde(tag = "result")]
#[serde(rename_all = "camelCase")]
enum CollectionResponse<T> {
    Ok { data: Vec<T> },
    Error { errors: Vec<ApiError> },
}

impl<T> CollectionResponse<T> {
    /// Converts this response into a [Result].
    fn into_result(self) -> Result<Vec<T>> {
        match self {
            CollectionResponse::Ok { data } => Ok(data),
            CollectionResponse::Error { errors } => Err(Error::Api(errors)),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manga {
    pub id: String,
    pub attributes: MangaAttributes,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MangaAttributes {
    pub title: HashMap<String, String>,
}

impl MangaAttributes {
    /// Gets the english title for this manga if it exists.
    pub fn english_title(&self) -> Option<&str> {
        self.title.get("en").map(|x| x.as_str())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub attributes: ChapterAttributes,
}

impl Chapter {
    pub fn url(&self) -> Url {
        Url::parse("https://mangadex.org")
            .unwrap()
            .join("/chapter/")
            .unwrap()
            .join(&self.id)
            .unwrap()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterAttributes {
    pub title: Option<String>,
    pub volume: Option<String>,
    pub chapter: Option<String>,
    pub pages: i32,
    pub translated_language: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub published_at: Option<String>,
    pub readable_at: Option<String>,
}

/// Retrieves the english title for a manga with a given id.
pub async fn english_title(manga_id: MangaId) -> Result<Option<String>> {
    let url = Url::parse(SITE)
        .unwrap()
        .join("/manga/")
        .unwrap()
        .join(&manga_id.to_string())
        .unwrap();

    let manga = fetch_json::<EntityResponse<Manga>>(url)
        .await?
        .into_result()
        .map_err(log_error)?;
    let title = manga.attributes.english_title().map(|s| s.to_owned());
    Ok(title)
}

/// Fetches the latest chapter for a given manga.
pub async fn latest_chapter(manga_id: MangaId) -> Result<Option<Chapter>> {
    let url = latest_chapter_url(manga_id);

    let mut chapter = fetch_json::<CollectionResponse<Chapter>>(url)
        .await?
        .into_result()
        .map_err(log_error)?;

    Ok(chapter.pop())
}

/// Fetches the latest chapter for a given manga only returning it if it's id differs
/// from the some previous latest chapter id.
pub async fn updated_chapter(
    manga_id: MangaId,
    latest_chapter_id: LatestChapterId,
) -> Result<Option<Chapter>> {
    let chapter = latest_chapter(manga_id).await?.and_then(|c| {
        let id = Uuid::try_parse(&c.id).unwrap();
        if Some(id) != latest_chapter_id {
            Some(c)
        } else {
            None
        }
    });

    Ok(chapter)
}

/// Constructs a URL that fetches the latest chapter for a given manga.
fn latest_chapter_url(manga_id: MangaId) -> Url {
    let mut url = Url::parse(SITE).unwrap().join("/chapter").unwrap();
    url.query_pairs_mut()
        .append_pair("manga", &manga_id.to_string())
        .append_pair("limit", "1")
        .append_pair("translatedLanguage[]", "en")
        .append_pair("contentRating[]", "safe")
        .append_pair("contentRating[]", "suggestive")
        .append_pair("order[chapter]", "desc");
    url
}

/// Sends an HTTP GET request to a given url decoding the response, if successful, from JSON.
async fn fetch_json<T>(url: Url) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let resp = reqwest::get(url.clone())
        .await
        .map_err(|err| err.with_url(url.clone()))
        .map_err(network_error)?;

    resp.json::<T>()
        .await
        .map_err(|err| err.with_url(url))
        .map_err(network_error)
}

/// Converts a [reqwest::Error] into a [crate::mangadex::Error].
fn network_error(err: reqwest::Error) -> Error {
    log::error!("reqwest error: {err}");
    Error::NetworkError
}

/// Logs an error returning it as is. Intended to be used via as a part of a call chain.
fn log_error(err: Error) -> Error {
    match &err {
        Error::NetworkError => log::error!("A network error occurred"),
        Error::Api(errors) => {
            for err in errors {
                log::error!(
                    "mangadex error: id = {}, status = {}, title = {}, details = {}",
                    err.id,
                    err.status,
                    err.title,
                    err.detail
                );
            }
        }
    }

    err
}
