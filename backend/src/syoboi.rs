use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Context;
use chrono::{FixedOffset, TimeZone};
use reqwest::Client;
use serde::Deserialize;

use crate::{
    config::SyoboiConfig,
    types::{SubjectCardDto, SubjectDetailDto},
};

#[derive(Clone)]
pub struct SyoboiClient {
    base_url: String,
    http: Client,
    cache: Arc<Mutex<HashMap<String, Option<String>>>>,
}

impl SyoboiClient {
    pub fn new(config: &SyoboiConfig) -> anyhow::Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .context("failed to build syoboi http client")?;

        Ok(Self {
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            http,
            cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn enrich_card(&self, mut card: SubjectCardDto) -> SubjectCardDto {
        card.broadcast_time = self
            .resolve_broadcast_time(&card.title, &card.title_cn, card.air_date.as_deref())
            .await;
        card
    }

    pub async fn enrich_detail(&self, mut detail: SubjectDetailDto) -> SubjectDetailDto {
        detail.broadcast_time = self
            .resolve_broadcast_time(&detail.title, &detail.title_cn, detail.air_date.as_deref())
            .await;
        detail
    }

    async fn resolve_broadcast_time(
        &self,
        title: &str,
        title_cn: &str,
        air_date: Option<&str>,
    ) -> Option<String> {
        let cache_key = build_cache_key(title, title_cn, air_date);

        if let Some(cached) = self
            .cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&cache_key).cloned())
        {
            return cached;
        }

        let mut resolved = None;
        for term in build_search_terms(title, title_cn) {
            match self.search_titles(&term).await {
                Ok(response) => {
                    if let Some(value) = select_broadcast_time(&response, title, title_cn, air_date)
                    {
                        resolved = Some(value);
                        break;
                    }
                }
                Err(error) => {
                    tracing::warn!("failed to resolve Syoboi airtime for '{}': {}", title, error);
                }
            }
        }

        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(cache_key, resolved.clone());
        }

        resolved
    }

    async fn search_titles(&self, term: &str) -> anyhow::Result<TitleSearchResponseRaw> {
        self.http
            .get(format!("{}/json.php", self.base_url))
            .query(&[
                ("Req", "TitleSearch"),
                ("Search", term),
                ("Limit", "10"),
            ])
            .send()
            .await
            .with_context(|| format!("failed to reach Syoboi for search term '{}'", term))?
            .error_for_status()
            .with_context(|| format!("Syoboi returned an error for search term '{}'", term))?
            .json::<TitleSearchResponseRaw>()
            .await
            .with_context(|| format!("failed to parse Syoboi response for '{}'", term))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TitleSearchResponseRaw {
    #[serde(rename = "Titles", default)]
    titles: Option<HashMap<String, SyoboiTitleRaw>>,
}

#[derive(Debug, Clone, Deserialize)]
struct SyoboiTitleRaw {
    #[serde(rename = "Title", default)]
    title: String,
    #[serde(rename = "ShortTitle", default)]
    short_title: String,
    #[serde(rename = "FirstYear", default)]
    first_year: Option<String>,
    #[serde(rename = "FirstMonth", default)]
    first_month: Option<String>,
    #[serde(rename = "FirstCh", default)]
    first_ch: Option<String>,
    #[serde(rename = "Search", default)]
    search: Option<i64>,
    #[serde(rename = "Programs", default)]
    programs: Vec<SyoboiProgramRaw>,
}

#[derive(Debug, Clone, Deserialize)]
struct SyoboiProgramRaw {
    #[serde(rename = "StTime")]
    start_time: FlexibleValue,
    #[serde(rename = "ChName", default)]
    channel_name: String,
    #[serde(rename = "Count", default)]
    count: Option<FlexibleValue>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum FlexibleValue {
    Text(String),
    Integer(i64),
    Float(f64),
}

impl FlexibleValue {
    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Text(value) => value.parse::<i64>().ok(),
            Self::Integer(value) => Some(*value),
            Self::Float(value) => Some(*value as i64),
        }
    }

    fn is_present(&self) -> bool {
        match self {
            Self::Text(value) => !value.trim().is_empty(),
            Self::Integer(_) | Self::Float(_) => true,
        }
    }
}

fn build_cache_key(title: &str, title_cn: &str, air_date: Option<&str>) -> String {
    let year_month = air_date
        .and_then(|value| value.split_once('T').map(|(left, _)| left).or(Some(value)))
        .and_then(|value| value.get(..7))
        .unwrap_or_default();

    format!(
        "{}|{}|{}",
        normalize_title(title),
        normalize_title(title_cn),
        year_month
    )
}

fn build_search_terms(title: &str, title_cn: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for candidate in [title.trim(), title_cn.trim()] {
        if candidate.is_empty() {
            continue;
        }

        let normalized = normalize_title(candidate);
        if terms
            .iter()
            .any(|existing| normalize_title(existing) == normalized)
        {
            continue;
        }

        terms.push(candidate.to_owned());
    }

    terms
}

fn select_broadcast_time(
    response: &TitleSearchResponseRaw,
    title: &str,
    title_cn: &str,
    air_date: Option<&str>,
) -> Option<String> {
    let candidates = response.titles.as_ref()?.values().collect::<Vec<_>>();
    let (air_year, air_month) = extract_year_month(air_date);

    let best_title = candidates
        .into_iter()
        .filter(|candidate| !candidate.programs.is_empty())
        .max_by_key(|candidate| candidate_score(candidate, title, title_cn, air_year, air_month))?;

    let program = pick_program(best_title)?;
    let timestamp = program.start_time.as_i64()?;
    format_jst_time(timestamp)
}

fn candidate_score(
    candidate: &SyoboiTitleRaw,
    title: &str,
    title_cn: &str,
    air_year: Option<i32>,
    air_month: Option<u32>,
) -> i32 {
    let primary = normalize_title(title);
    let secondary = normalize_title(title_cn);
    let title_text = normalize_title(&candidate.title);
    let short_text = normalize_title(&candidate.short_title);
    let mut score = 0;

    score += title_match_score(&title_text, &primary, 120, 52);
    score += title_match_score(&short_text, &primary, 108, 44);
    score += title_match_score(&title_text, &secondary, 92, 40);
    score += title_match_score(&short_text, &secondary, 84, 34);
    score += candidate.search.unwrap_or_default() as i32 * 12;

    if air_year == parse_i32(candidate.first_year.as_deref()) {
        score += 18;
    }

    if air_month == parse_u32(candidate.first_month.as_deref()) {
        score += 8;
    }

    if !candidate.programs.is_empty() {
        score += 16;
    }

    score
}

fn title_match_score(candidate: &str, target: &str, exact_score: i32, partial_score: i32) -> i32 {
    if candidate.is_empty() || target.is_empty() {
        return 0;
    }

    if candidate == target {
        return exact_score;
    }

    if candidate.contains(target) || target.contains(candidate) {
        return partial_score;
    }

    0
}

fn pick_program(candidate: &SyoboiTitleRaw) -> Option<&SyoboiProgramRaw> {
    let preferred_channels = candidate
        .first_ch
        .as_deref()
        .map(split_channels)
        .unwrap_or_default();

    candidate.programs.iter().min_by_key(|program| {
        let channel_match = preferred_channels.iter().any(|channel| {
            !channel.is_empty()
                && (program.channel_name.contains(channel) || channel.contains(&program.channel_name))
        });

        (
            if channel_match { 0 } else { 1 },
            if program.count.as_ref().is_some_and(FlexibleValue::is_present) {
                0
            } else {
                1
            },
            program.start_time.as_i64().unwrap_or(i64::MAX),
        )
    })
}

fn split_channels(value: &str) -> Vec<String> {
    value
        .split(['、', ',', '，', '/', '／'])
        .map(str::trim)
        .filter(|channel| !channel.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn format_jst_time(timestamp: i64) -> Option<String> {
    let timezone = FixedOffset::east_opt(9 * 3600)?;
    let datetime = timezone.timestamp_opt(timestamp, 0).single()?;
    Some(datetime.format("%H:%M").to_string())
}

fn extract_year_month(air_date: Option<&str>) -> (Option<i32>, Option<u32>) {
    let Some(value) = air_date else {
        return (None, None);
    };

    let date_part = value.split_once('T').map(|(left, _)| left).unwrap_or(value);
    let mut segments = date_part.split('-');
    let year = segments.next().and_then(|segment| segment.parse::<i32>().ok());
    let month = segments.next().and_then(|segment| segment.parse::<u32>().ok());

    (year, month)
}

fn parse_i32(value: Option<&str>) -> Option<i32> {
    value.and_then(|segment| segment.parse::<i32>().ok())
}

fn parse_u32(value: Option<&str>) -> Option<u32> {
    value.and_then(|segment| segment.parse::<u32>().ok())
}

fn normalize_title(value: &str) -> String {
    let mut normalized = String::new();

    for character in value.chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
        }
    }

    normalized
}
