use serde::Deserialize;
use url::Url;

const SITE: &'static str = "https://api.mangadex.org";

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub result: String,
    pub response: String,
    pub data: T,
    pub limit: i32,
    pub offset: i32,
    pub total: i32,
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
