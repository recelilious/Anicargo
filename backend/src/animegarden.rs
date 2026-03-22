use std::time::Duration;

use anyhow::Context;
use reqwest::Client;
use serde::Deserialize;
use tracing::warn;

use crate::{config::AnimeGardenConfig, types::AppError};

#[derive(Clone)]
pub struct AnimeGardenClient {
    base_url: String,
    http: Client,
    page_size: usize,
    max_pages: usize,
}

#[derive(Debug, Clone)]
pub struct AnimeGardenSearchProfile {
    pub bangumi_subject_id: i64,
    pub title: String,
    pub title_cn: String,
}

#[derive(Debug, Clone)]
pub struct AnimeGardenSearchResult {
    pub strategy: String,
    pub resources: Vec<AnimeGardenResource>,
}

#[derive(Debug, Clone)]
pub struct AnimeGardenResource {
    pub provider: String,
    pub provider_id: String,
    pub title: String,
    pub href: String,
    pub release_type: String,
    pub magnet: String,
    pub size: i64,
    pub created_at: String,
    pub fetched_at: String,
    pub fansub_name: Option<String>,
    pub publisher_name: String,
}

impl AnimeGardenClient {
    pub fn new(config: &AnimeGardenConfig) -> anyhow::Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .context("failed to build animegarden http client")?;

        Ok(Self {
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            http,
            page_size: config.page_size.max(1),
            max_pages: config.max_pages.max(1),
        })
    }

    pub async fn search_resources(
        &self,
        profile: &AnimeGardenSearchProfile,
    ) -> Result<AnimeGardenSearchResult, AppError> {
        let by_subject = self
            .fetch_resources(build_subject_params(profile.bangumi_subject_id))
            .await?;
        if !by_subject.is_empty() {
            return Ok(AnimeGardenSearchResult {
                strategy: "subject".to_owned(),
                resources: by_subject,
            });
        }

        if let Some(keyword) = preferred_search_term(profile) {
            let by_search = self
                .fetch_resources(build_search_params(keyword.clone()))
                .await?;
            if !by_search.is_empty() {
                return Ok(AnimeGardenSearchResult {
                    strategy: format!("search:{keyword}"),
                    resources: by_search,
                });
            }
        }

        if let Some(keyword) = secondary_search_term(profile) {
            let by_search = self
                .fetch_resources(build_search_params(keyword.clone()))
                .await?;
            if !by_search.is_empty() {
                return Ok(AnimeGardenSearchResult {
                    strategy: format!("search:{keyword}"),
                    resources: by_search,
                });
            }
        }

        Ok(AnimeGardenSearchResult {
            strategy: "subject_then_search".to_owned(),
            resources: Vec::new(),
        })
    }

    async fn fetch_resources(
        &self,
        extra_params: Vec<(String, String)>,
    ) -> Result<Vec<AnimeGardenResource>, AppError> {
        let mut merged = Vec::new();

        for page in 1..=self.max_pages {
            let mut query = vec![
                ("page".to_owned(), page.to_string()),
                ("pageSize".to_owned(), self.page_size.to_string()),
            ];
            query.extend(extra_params.clone());

            let url = format!("{}/resources", self.base_url);
            let response = self
                .http
                .get(&url)
                .query(&query)
                .send()
                .await
                .map_err(|error| {
                    warn!(url = %url, page, error = %error, "Failed to reach AnimeGarden resources");
                    AppError::upstream("failed to reach AnimeGarden resources")
                })?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                warn!(
                    url = %url,
                    page,
                    status = %status,
                    body = %body.chars().take(200).collect::<String>(),
                    "AnimeGarden resources returned an unsuccessful response"
                );
                return Err(AppError::upstream(
                    "AnimeGarden resources returned an error",
                ));
            }

            let payload = response
                .json::<ResourceListResponse>()
                .await
                .map_err(|error| {
                    warn!(url = %url, page, error = %error, "Failed to parse AnimeGarden resources response");
                    AppError::upstream("failed to parse AnimeGarden resources response")
                })?;

            let is_empty = payload.resources.is_empty();
            merged.extend(payload.resources.into_iter().map(AnimeGardenResource::from));

            if payload.pagination.complete || is_empty {
                break;
            }
        }

        Ok(merged)
    }
}

fn build_subject_params(subject_id: i64) -> Vec<(String, String)> {
    vec![("subject".to_owned(), subject_id.to_string())]
}

fn build_search_params(keyword: String) -> Vec<(String, String)> {
    vec![("search".to_owned(), keyword)]
}

fn preferred_search_term(profile: &AnimeGardenSearchProfile) -> Option<String> {
    sanitize_search_term(&profile.title_cn).or_else(|| sanitize_search_term(&profile.title))
}

fn secondary_search_term(profile: &AnimeGardenSearchProfile) -> Option<String> {
    let primary = preferred_search_term(profile);
    let secondary =
        sanitize_search_term(&profile.title).or_else(|| sanitize_search_term(&profile.title_cn));
    match (primary, secondary) {
        (Some(primary), Some(secondary)) if primary != secondary => Some(secondary),
        _ => None,
    }
}

fn sanitize_search_term(value: &str) -> Option<String> {
    let term = value
        .replace(
            [
                '：', ':', '～', '~', '！', '!', '？', '?', '「', '」', '『', '』',
            ],
            " ",
        )
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!term.is_empty()).then_some(term)
}

#[derive(Debug, Deserialize)]
struct ResourceListResponse {
    #[serde(default)]
    resources: Vec<ResourceRaw>,
    pagination: PaginationRaw,
}

#[derive(Debug, Deserialize)]
struct PaginationRaw {
    complete: bool,
}

#[derive(Debug, Deserialize)]
struct ResourceRaw {
    provider: String,
    #[serde(rename = "providerId")]
    provider_id: String,
    title: String,
    href: String,
    #[serde(rename = "type")]
    release_type: String,
    magnet: String,
    size: i64,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "fetchedAt")]
    fetched_at: String,
    fansub: Option<FansubRaw>,
    publisher: PublisherRaw,
}

#[derive(Debug, Deserialize)]
struct FansubRaw {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PublisherRaw {
    name: String,
}

impl From<ResourceRaw> for AnimeGardenResource {
    fn from(value: ResourceRaw) -> Self {
        Self {
            provider: value.provider,
            provider_id: value.provider_id,
            title: value.title,
            href: value.href,
            release_type: value.release_type,
            magnet: value.magnet,
            size: value.size,
            created_at: value.created_at,
            fetched_at: value.fetched_at,
            fansub_name: value.fansub.map(|fansub| fansub.name),
            publisher_name: value.publisher.name,
        }
    }
}
