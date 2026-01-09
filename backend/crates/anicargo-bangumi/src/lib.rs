use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

const BASE_URL: &str = "https://api.bgm.tv";

#[derive(Debug)]
pub enum BangumiError {
    Http(reqwest::Error),
    InvalidHeader(String),
    InvalidInput(String),
}

impl fmt::Display for BangumiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BangumiError::Http(err) => write!(f, "http error: {}", err),
            BangumiError::InvalidHeader(message) => write!(f, "invalid header: {}", message),
            BangumiError::InvalidInput(message) => write!(f, "invalid input: {}", message),
        }
    }
}

impl std::error::Error for BangumiError {}

impl From<reqwest::Error> for BangumiError {
    fn from(err: reqwest::Error) -> Self {
        BangumiError::Http(err)
    }
}

#[derive(Debug, Clone)]
pub struct BangumiClient {
    client: reqwest::Client,
    base_url: String,
}

impl BangumiClient {
    pub fn new(access_token: Option<String>, user_agent: String) -> Result<Self, BangumiError> {
        let mut headers = HeaderMap::new();
        if let Some(token) = access_token {
            let value = format!("Bearer {}", token);
            let header_value = HeaderValue::from_str(&value)
                .map_err(|_| BangumiError::InvalidHeader("authorization".to_string()))?;
            headers.insert(AUTHORIZATION, header_value);
        }

        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .default_headers(headers)
            .build()
            .map_err(BangumiError::Http)?;

        Ok(Self {
            client,
            base_url: BASE_URL.to_string(),
        })
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    pub async fn search_anime(
        &self,
        keyword: &str,
        limit: u32,
    ) -> Result<Paged<Subject>, BangumiError> {
        let keyword = keyword.trim();
        if keyword.is_empty() {
            return Err(BangumiError::InvalidInput(
                "keyword must not be empty".to_string(),
            ));
        }

        let request = SearchRequest {
            keyword: keyword.to_string(),
            sort: Some("match".to_string()),
            filter: SearchFilter {
                types: vec![2],
            },
        };

        let url = format!("{}/v0/search/subjects", self.base_url);
        let response = self
            .client
            .post(url)
            .query(&[("limit", limit), ("offset", 0u32)])
            .json(&request)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json::<Paged<Subject>>().await?)
    }

    pub async fn get_subject(&self, subject_id: i64) -> Result<Subject, BangumiError> {
        let url = format!("{}/v0/subjects/{}", self.base_url, subject_id);
        let response = self.client.get(url).send().await?.error_for_status()?;
        Ok(response.json::<Subject>().await?)
    }

    pub async fn get_all_episodes(
        &self,
        subject_id: i64,
    ) -> Result<Vec<Episode>, BangumiError> {
        let mut offset = 0;
        let mut episodes = Vec::new();

        loop {
            let page = self.get_episode_page(subject_id, offset).await?;
            episodes.extend(page.data);
            offset += page.limit as i64;
            if offset >= page.total as i64 || page.limit == 0 {
                break;
            }
        }

        Ok(episodes)
    }

    async fn get_episode_page(
        &self,
        subject_id: i64,
        offset: i64,
    ) -> Result<Paged<Episode>, BangumiError> {
        let url = format!("{}/v0/episodes", self.base_url);
        let response = self
            .client
            .get(url)
            .query(&[
                ("subject_id", subject_id.to_string()),
                ("limit", "200".to_string()),
                ("offset", offset.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json::<Paged<Episode>>().await?)
    }
}

#[derive(Debug, Serialize)]
struct SearchRequest {
    keyword: String,
    sort: Option<String>,
    filter: SearchFilter,
}

#[derive(Debug, Serialize)]
struct SearchFilter {
    #[serde(rename = "type")]
    types: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct Paged<T> {
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub data: Vec<T>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Subject {
    pub id: i64,
    #[serde(rename = "type")]
    pub subject_type: i32,
    pub name: String,
    #[serde(rename = "name_cn")]
    pub name_cn: String,
    pub summary: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub total_episodes: Option<i64>,
    #[serde(default)]
    pub images: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Episode {
    pub id: i64,
    #[serde(rename = "type")]
    pub episode_type: i32,
    pub name: String,
    #[serde(rename = "name_cn")]
    pub name_cn: String,
    pub sort: f64,
    #[serde(default)]
    pub ep: Option<f64>,
    #[serde(default)]
    pub airdate: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_keyword_is_rejected() {
        let client = BangumiClient::new(None, "Anicargo-test/0.1".to_string()).unwrap();
        let err = client.search_anime("  ", 5).await.unwrap_err();
        assert!(matches!(err, BangumiError::InvalidInput(_)));
    }
}
