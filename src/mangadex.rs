use std::collections::HashMap;

use serde::Deserialize;
use url::Url;
use uuid::Uuid;

const SITE: &'static str = "https://api.mangadex.org";

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub result: String,
    pub response: String,
    pub data: T,
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
pub async fn english_title(manga_id: Uuid) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let url = Url::parse(SITE)
        .unwrap()
        .join("/manga/")
        .unwrap()
        .join(&manga_id.to_string())
        .unwrap();

    let resp = reqwest::get(url)
        .await?
        .json::<ApiResponse<Manga>>()
        .await?;

    let title = resp.data.attributes.english_title().map(|s| s.to_owned());
    Ok(title)
}

fn latest_chapter_url(manga_id: &str) -> Url {
    let mut url = Url::parse(SITE).unwrap().join("/chapter").unwrap();
    url.query_pairs_mut()
        .append_pair("manga", manga_id)
        .append_pair("limit", "1")
        .append_pair("translatedLanguage[]", "en")
        .append_pair("contentRating[]", "safe")
        .append_pair("contentRating[]", "suggestive")
        .append_pair("order[chapter]", "desc");
    url
}

pub async fn print_latest_chapter() -> Result<Option<Chapter>, Box<dyn std::error::Error>> {
    let url = latest_chapter_url("26e40241-4a4e-4d12-a04d-cb3f7f707100");
    let resp = reqwest::get(url)
        .await?
        .json::<ApiResponse<Vec<Chapter>>>()
        .await?;

    let chapter = resp.data.first().cloned();
    Ok(chapter)
}
