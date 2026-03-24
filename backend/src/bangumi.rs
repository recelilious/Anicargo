use std::time::Duration;

use anyhow::Context;
use chrono::{Local, NaiveDate};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tracing::warn;

use crate::{
    config::BangumiConfig,
    types::{AppError, EpisodeDto, InfoboxItemDto, SubjectCardDto, SubjectDetailDto},
};

#[derive(Clone)]
pub struct BangumiClient {
    base_url: String,
    http: Client,
    user_agent: String,
}

impl BangumiClient {
    pub fn new(config: &BangumiConfig) -> anyhow::Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .context("failed to build bangumi http client")?;

        Ok(Self {
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            http,
            user_agent: config.user_agent.clone(),
        })
    }

    pub async fn search_subjects(
        &self,
        request: &BangumiSearchQuery,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponseRaw, AppError> {
        let payload = request.to_payload();
        let url = format!(
            "{}/v0/search/subjects?limit={}&offset={}",
            self.base_url, limit, offset
        );

        let response = self
            .send_request(
                self.http
                    .post(&url)
                    .header(reqwest::header::USER_AGENT, &self.user_agent)
                    .json(&payload),
                "search",
                &url,
            )
            .await?;

        if !response.status().is_success() {
            return Err(self.search_status_error(response, &url).await);
        }

        response.json::<SearchResponseRaw>().await.map_err(|error| {
            warn!(url = %url, error = %error, "Failed to parse Bangumi search response");
            AppError::upstream("failed to parse Bangumi search response")
        })
    }

    pub async fn fetch_subject(&self, subject_id: i64) -> Result<SubjectRaw, AppError> {
        let url = format!("{}/v0/subjects/{}", self.base_url, subject_id);
        let response = self
            .send_request(
                self.http
                    .get(&url)
                    .header(reqwest::header::USER_AGENT, &self.user_agent),
                "subject detail",
                &url,
            )
            .await?;

        if !response.status().is_success() {
            return Err(self.subject_status_error(response, &url).await);
        }

        response.json::<SubjectRaw>().await.map_err(|error| {
            warn!(
                url = %url,
                subject_id,
                error = %error,
                "Failed to parse Bangumi subject detail response"
            );
            AppError::upstream("failed to parse Bangumi subject detail")
        })
    }

    pub async fn fetch_episodes(&self, subject_id: i64) -> Result<Vec<EpisodeRaw>, AppError> {
        let url = format!(
            "{}/v0/episodes?subject_id={}&type=0",
            self.base_url, subject_id
        );
        let response = self
            .send_request(
                self.http
                    .get(&url)
                    .header(reqwest::header::USER_AGENT, &self.user_agent),
                "episode list",
                &url,
            )
            .await?;

        if !response.status().is_success() {
            return Err(self.episodes_status_error(response, &url, subject_id).await);
        }

        response
            .json::<PagedEpisodesRaw>()
            .await
            .map_err(|error| {
                warn!(
                    url = %url,
                    subject_id,
                    error = %error,
                    "Failed to parse Bangumi episode list response"
                );
                AppError::upstream("failed to parse Bangumi episode list")
            })
            .map(|payload| payload.data)
    }

    async fn send_request(
        &self,
        request: reqwest::RequestBuilder,
        action: &str,
        url: &str,
    ) -> Result<Response, AppError> {
        request.send().await.map_err(|error| {
            warn!(action, url = %url, error = %error, "Failed to reach Bangumi");
            AppError::upstream(format!("failed to reach Bangumi {action}"))
        })
    }

    async fn search_status_error(&self, response: Response, url: &str) -> AppError {
        let (status, body) = read_upstream_error(response).await;
        warn!(
            url = %url,
            status = %status,
            body = %body,
            "Bangumi search returned an unsuccessful response"
        );

        match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                AppError::bad_request("Bangumi rejected the current search filters")
            }
            StatusCode::TOO_MANY_REQUESTS => {
                AppError::upstream("Bangumi search is temporarily rate limited")
            }
            _ => AppError::upstream("Bangumi search returned an error"),
        }
    }

    async fn subject_status_error(&self, response: Response, url: &str) -> AppError {
        let (status, body) = read_upstream_error(response).await;
        warn!(
            url = %url,
            status = %status,
            body = %body,
            "Bangumi subject detail returned an unsuccessful response"
        );

        if status == StatusCode::NOT_FOUND {
            AppError::not_found("subject not found on Bangumi")
        } else {
            AppError::upstream("Bangumi subject detail returned an error")
        }
    }

    async fn episodes_status_error(
        &self,
        response: Response,
        url: &str,
        subject_id: i64,
    ) -> AppError {
        let (status, body) = read_upstream_error(response).await;
        warn!(
            url = %url,
            subject_id,
            status = %status,
            body = %body,
            "Bangumi episode list returned an unsuccessful response"
        );
        AppError::upstream("Bangumi episode list returned an error")
    }
}

async fn read_upstream_error(response: Response) -> (StatusCode, String) {
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_default()
        .chars()
        .take(240)
        .collect::<String>();
    (status, body)
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponseRaw {
    #[serde(default)]
    pub data: Vec<SubjectRaw>,
    #[serde(default)]
    pub total: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct BangumiSearchQuery {
    pub keyword: String,
    pub sort: String,
    pub tags: Vec<String>,
    pub meta_tags: Vec<String>,
    pub air_date_start: Option<String>,
    pub air_date_end: Option<String>,
    pub rating_min: Option<f64>,
    pub rating_max: Option<f64>,
    pub rating_count_min: Option<u32>,
    pub rating_count_max: Option<u32>,
    pub rank_min: Option<u32>,
    pub rank_max: Option<u32>,
    pub nsfw: Option<bool>,
}

impl BangumiSearchQuery {
    fn to_payload(&self) -> Value {
        let mut filter = Map::new();
        filter.insert("type".to_owned(), json!([2]));

        if !self.tags.is_empty() {
            filter.insert("tag".to_owned(), json!(self.tags));
        }

        if !self.meta_tags.is_empty() {
            filter.insert("meta_tags".to_owned(), json!(self.meta_tags));
        }

        if let Some(values) = build_range_filter(
            self.air_date_start.as_deref(),
            self.air_date_end.as_deref(),
            None::<fn(&str) -> String>,
        ) {
            filter.insert("air_date".to_owned(), json!(values));
        }

        if let Some(values) = build_range_filter(
            self.rating_min,
            self.rating_max,
            Some(|value: f64| trim_float(value)),
        ) {
            filter.insert("rating".to_owned(), json!(values));
        }

        if let Some(values) = build_range_filter(
            self.rating_count_min,
            self.rating_count_max,
            Some(|value: u32| value.to_string()),
        ) {
            filter.insert("rating_count".to_owned(), json!(values));
        }

        if let Some(values) = build_range_filter(
            self.rank_min,
            self.rank_max,
            Some(|value: u32| value.to_string()),
        ) {
            filter.insert("rank".to_owned(), json!(values));
        }

        if let Some(nsfw) = self.nsfw {
            filter.insert("nsfw".to_owned(), json!(nsfw));
        }

        json!({
            "keyword": self.keyword,
            "sort": self.sort,
            "filter": Value::Object(filter),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubjectRaw {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub name_cn: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub air_weekday: Option<u8>,
    #[serde(default)]
    pub total_episodes: Option<i64>,
    #[serde(default)]
    pub images: Option<ImageSetRaw>,
    #[serde(default)]
    pub tags: Vec<TagRaw>,
    #[serde(default)]
    pub infobox: Vec<InfoboxRaw>,
    #[serde(default)]
    pub rating: Option<RatingRaw>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EpisodeRaw {
    pub id: i64,
    #[serde(default)]
    pub sort: Option<f64>,
    #[serde(default)]
    pub ep: Option<f64>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub name_cn: String,
    #[serde(default)]
    pub airdate: String,
    #[serde(default)]
    pub duration_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PagedEpisodesRaw {
    #[serde(default)]
    pub data: Vec<EpisodeRaw>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageSetRaw {
    #[serde(default)]
    pub large: Option<String>,
    #[serde(default)]
    pub common: Option<String>,
    #[serde(default)]
    pub medium: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TagRaw {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RatingRaw {
    #[serde(default)]
    pub score: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InfoboxRaw {
    pub key: String,
    #[serde(default)]
    pub value: Value,
}

impl SubjectRaw {
    pub fn to_card(&self) -> SubjectCardDto {
        let mut card = self.base_card();
        card.release_status = self.search_release_status().to_owned();
        card
    }

    fn base_card(&self) -> SubjectCardDto {
        let tags = self
            .tags
            .iter()
            .take(8)
            .map(|tag| tag.name.clone())
            .collect();

        SubjectCardDto {
            bangumi_subject_id: self.id,
            title: self.name.clone(),
            title_cn: self.name_cn.clone(),
            summary: self.summary.clone(),
            release_status: "completed".to_owned(),
            air_date: self.air_date.clone().or(self.date.clone()),
            broadcast_time: None,
            air_weekday: self.air_weekday,
            image_portrait: self.images.as_ref().and_then(|images| {
                images
                    .large
                    .clone()
                    .or(images.common.clone())
                    .or(images.medium.clone())
            }),
            image_banner: self
                .images
                .as_ref()
                .and_then(|images| images.common.clone().or(images.large.clone())),
            tags,
            total_episodes: self.total_episodes,
            rating_score: self.rating.as_ref().and_then(|rating| rating.score),
            catalog_label: None,
        }
    }

    pub fn to_detail(&self) -> SubjectDetailDto {
        SubjectDetailDto {
            bangumi_subject_id: self.id,
            title: self.name.clone(),
            title_cn: self.name_cn.clone(),
            summary: self.summary.clone(),
            air_date: self.air_date.clone().or(self.date.clone()),
            broadcast_time: None,
            air_weekday: self.air_weekday,
            total_episodes: self.total_episodes,
            image_portrait: self
                .images
                .as_ref()
                .and_then(|images| images.large.clone().or(images.common.clone())),
            image_banner: self
                .images
                .as_ref()
                .and_then(|images| images.common.clone().or(images.large.clone())),
            tags: self
                .tags
                .iter()
                .map(|tag| tag.name.clone())
                .take(8)
                .collect(),
            infobox: self
                .infobox
                .iter()
                .take(18)
                .map(|item| InfoboxItemDto {
                    key: item.key.clone(),
                    value: flatten_infobox_value(&item.value),
                })
                .filter(|item| !item.value.is_empty())
                .collect(),
            rating_score: self.rating.as_ref().and_then(|rating| rating.score),
        }
    }

    fn search_release_status(&self) -> &'static str {
        if self.is_upcoming() {
            return "upcoming";
        }

        if self.has_explicit_airing_marker() || self.has_fallback_airing_marker() {
            return "airing";
        }

        "completed"
    }

    fn is_upcoming(&self) -> bool {
        parse_subject_date(self.air_date.as_ref().or(self.date.as_ref()))
            .is_some_and(|date| date > Local::now().date_naive())
    }

    fn has_fallback_airing_marker(&self) -> bool {
        self.infobox.iter().any(|item| {
            let key = item.key.to_lowercase();
            let value = flatten_infobox_value(&item.value).to_lowercase();
            let combined = format!("{key} {value}");

            combined.contains("放送中")
                || combined.contains("播出中")
                || combined.contains("播放中")
                || combined.contains("连载中")
                || combined.contains("連載中")
                || combined.contains("更新中")
                || combined.contains("上映中")
                || combined.contains("配信中")
                || combined.contains("airing")
                || combined.contains("ongoing")
        })
    }

    fn has_explicit_airing_marker(&self) -> bool {
        self.infobox.iter().any(|item| {
            let key = item.key.to_lowercase();
            let value = flatten_infobox_value(&item.value).to_lowercase();
            let combined = format!("{key} {value}");

            combined.contains("放送中")
                || combined.contains("播出中")
                || combined.contains("播放中")
                || combined.contains("连载中")
                || combined.contains("連載中")
                || combined.contains("上映中")
                || combined.contains("配信中")
                || combined.contains("airing")
                || combined.contains("ongoing")
        })
    }
}

impl EpisodeRaw {
    pub fn preferred_episode_number(&self) -> Option<f64> {
        self.ep.or(self.sort).filter(|value| *value > 0.0)
    }

    pub fn to_dto(&self, is_available: bool, availability_note: Option<String>) -> EpisodeDto {
        EpisodeDto {
            bangumi_episode_id: self.id,
            sort: self.sort.unwrap_or_default(),
            episode_number: self.ep,
            title: self.name.clone(),
            title_cn: self.name_cn.clone(),
            airdate: if self.airdate.is_empty() {
                None
            } else {
                Some(self.airdate.clone())
            },
            duration_seconds: self.duration_seconds,
            is_available,
            availability_note,
        }
    }
}

fn flatten_infobox_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Array(items) => items
            .iter()
            .map(flatten_infobox_value)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" / "),
        Value::Object(map) => map
            .get("v")
            .map(flatten_infobox_value)
            .or_else(|| map.get("name").map(flatten_infobox_value))
            .unwrap_or_default(),
    }
}

fn parse_subject_date(value: Option<&String>) -> Option<NaiveDate> {
    let date = value?;
    let date_part = date.split_once('T').map(|(left, _)| left).unwrap_or(date);
    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}

fn build_range_filter<T, F>(
    min: Option<T>,
    max: Option<T>,
    formatter: Option<F>,
) -> Option<Vec<String>>
where
    T: Clone + ToString,
    F: Fn(T) -> String,
{
    let mut values = Vec::new();

    if let Some(min) = min {
        let value = formatter
            .as_ref()
            .map(|format| format(min.clone()))
            .unwrap_or_else(|| min.to_string());
        values.push(format!(">={value}"));
    }

    if let Some(max) = max {
        let value = formatter
            .as_ref()
            .map(|format| format(max.clone()))
            .unwrap_or_else(|| max.to_string());
        values.push(format!("<={value}"));
    }

    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn trim_float(value: f64) -> String {
    let mut text = format!("{value:.2}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFacets {
    pub years: Vec<i32>,
    pub tags: Vec<String>,
}
