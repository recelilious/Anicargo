use std::time::Duration;

use anyhow::Context;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    config::BangumiConfig,
    types::{AppError, EpisodeDto, InfoboxItemDto, SubjectCardDto, SubjectDetailDto, WeekdayDto},
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

    pub async fn fetch_calendar(&self) -> Result<Vec<CalendarDayRaw>, AppError> {
        self.http
            .get(format!("{}/calendar", self.base_url))
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .send()
            .await
            .map_err(|_| AppError::upstream("failed to reach Bangumi calendar"))?
            .error_for_status()
            .map_err(|_| AppError::upstream("Bangumi calendar returned an error"))?
            .json::<Vec<CalendarDayRaw>>()
            .await
            .map_err(|_| AppError::upstream("failed to parse Bangumi calendar response"))
    }

    pub async fn search_subjects(
        &self,
        keyword: &str,
        offset: usize,
    ) -> Result<SearchResponseRaw, AppError> {
        let payload = json!({
            "keyword": keyword,
            "sort": "rank",
            "filter": {
                "type": [2]
            }
        });

        self.http
            .post(format!(
                "{}/v0/search/subjects?limit=20&offset={}",
                self.base_url, offset
            ))
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .json(&payload)
            .send()
            .await
            .map_err(|_| AppError::upstream("failed to reach Bangumi search"))?
            .error_for_status()
            .map_err(|_| AppError::upstream("Bangumi search returned an error"))?
            .json::<SearchResponseRaw>()
            .await
            .map_err(|_| AppError::upstream("failed to parse Bangumi search response"))
    }

    pub async fn fetch_subject(&self, subject_id: i64) -> Result<SubjectRaw, AppError> {
        self.http
            .get(format!("{}/v0/subjects/{}", self.base_url, subject_id))
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .send()
            .await
            .map_err(|_| AppError::upstream("failed to reach Bangumi subject detail"))?
            .error_for_status()
            .map_err(|_| AppError::not_found("subject not found on Bangumi"))?
            .json::<SubjectRaw>()
            .await
            .map_err(|_| AppError::upstream("failed to parse Bangumi subject detail"))
    }

    pub async fn fetch_episodes(&self, subject_id: i64) -> Result<Vec<EpisodeRaw>, AppError> {
        self.http
            .get(format!(
                "{}/v0/episodes?subject_id={}&type=0",
                self.base_url, subject_id
            ))
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .send()
            .await
            .map_err(|_| AppError::upstream("failed to reach Bangumi episode list"))?
            .error_for_status()
            .map_err(|_| AppError::upstream("Bangumi episode list returned an error"))?
            .json::<PagedEpisodesRaw>()
            .await
            .map_err(|_| AppError::upstream("failed to parse Bangumi episode list"))
            .map(|response| response.data)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CalendarDayRaw {
    pub weekday: WeekdayRaw,
    pub items: Vec<SubjectRaw>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeekdayRaw {
    pub id: u8,
    pub cn: String,
    pub en: String,
    pub ja: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponseRaw {
    #[serde(default)]
    pub data: Vec<SubjectRaw>,
    #[serde(default)]
    pub total: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
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

impl CalendarDayRaw {
    pub fn to_weekday(&self) -> WeekdayDto {
        WeekdayDto {
            id: self.weekday.id,
            cn: self.weekday.cn.clone(),
            en: self.weekday.en.clone(),
            ja: self.weekday.ja.clone(),
        }
    }
}

impl SubjectRaw {
    pub fn to_card(&self) -> SubjectCardDto {
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
            release_status: self.release_status().to_owned(),
            air_date: self.air_date.clone().or(self.date.clone()),
            air_weekday: self.air_weekday,
            image_portrait: self
                .images
                .as_ref()
                .and_then(|images| images.large.clone().or(images.common.clone()).or(images.medium.clone())),
            image_banner: self
                .images
                .as_ref()
                .and_then(|images| images.common.clone().or(images.large.clone())),
            tags,
            total_episodes: self.total_episodes,
            rating_score: self.rating.as_ref().and_then(|rating| rating.score),
        }
    }

    pub fn to_detail(&self) -> SubjectDetailDto {
        SubjectDetailDto {
            bangumi_subject_id: self.id,
            title: self.name.clone(),
            title_cn: self.name_cn.clone(),
            summary: self.summary.clone(),
            air_date: self.air_date.clone().or(self.date.clone()),
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
            tags: self.tags.iter().map(|tag| tag.name.clone()).take(8).collect(),
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

    fn release_status(&self) -> &'static str {
        let has_end_marker = self.infobox.iter().any(|item| {
            let key = item.key.to_lowercase();
            let value = flatten_infobox_value(&item.value);

            !value.is_empty()
                && (key.contains("结束")
                    || key.contains("完结")
                    || key.contains("終了")
                    || key.contains("final")
                    || key.contains("终了"))
        });

        if has_end_marker {
            "completed"
        } else {
            "airing"
        }
    }
}

impl EpisodeRaw {
    pub fn to_dto(&self) -> EpisodeDto {
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
            is_available: false,
            availability_note: Some("资源尚未入库，后续会由订阅和下载规则驱动。".to_owned()),
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFacets {
    pub years: Vec<i32>,
    pub tags: Vec<String>,
}
