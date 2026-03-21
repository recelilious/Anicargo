use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Context;
use chrono::{FixedOffset, TimeZone};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    config::AniListConfig,
    types::{SubjectCardDto, SubjectDetailDto},
};

const AIRTIME_QUERY: &str = r#"
query ($search: String) {
  Media(search: $search, type: ANIME) {
    title {
      romaji
      english
      native
    }
    nextAiringEpisode {
      airingAt
      episode
    }
    seasonYear
    startDate {
      year
      month
      day
    }
    status
  }
}
"#;

#[derive(Clone)]
pub struct AniListClient {
    base_url: String,
    http: Client,
    cache: Arc<Mutex<HashMap<String, Option<String>>>>,
}

impl AniListClient {
    pub fn new(config: &AniListConfig) -> anyhow::Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .context("failed to build AniList http client")?;

        Ok(Self {
            base_url: config.base_url.clone(),
            http,
            cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn enrich_card(&self, mut card: SubjectCardDto) -> SubjectCardDto {
        if card.broadcast_time.is_some() {
            return card;
        }

        card.broadcast_time = self
            .resolve_broadcast_time(&card.title, &card.title_cn, card.air_date.as_deref())
            .await;
        card
    }

    pub async fn enrich_detail(&self, mut detail: SubjectDetailDto) -> SubjectDetailDto {
        if detail.broadcast_time.is_some() {
            return detail;
        }

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
            match self.search_title(&term).await {
                Ok(response) => {
                    if let Some(value) = select_broadcast_time(&response, title, title_cn, air_date)
                    {
                        resolved = Some(value);
                        break;
                    }
                }
                Err(error) => {
                    tracing::warn!("failed to resolve AniList airtime for '{}': {}", title, error);
                }
            }
        }

        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(cache_key, resolved.clone());
        }

        resolved
    }

    async fn search_title(&self, term: &str) -> anyhow::Result<AniListResponseRaw> {
        self.http
            .post(&self.base_url)
            .json(&AniListRequest {
                query: AIRTIME_QUERY,
                variables: AniListVariables { search: term },
            })
            .send()
            .await
            .with_context(|| format!("failed to reach AniList for search term '{}'", term))?
            .error_for_status()
            .with_context(|| format!("AniList returned an error for search term '{}'", term))?
            .json::<AniListResponseRaw>()
            .await
            .with_context(|| format!("failed to parse AniList response for '{}'", term))
    }
}

#[derive(Debug, Serialize)]
struct AniListRequest<'a> {
    query: &'a str,
    variables: AniListVariables<'a>,
}

#[derive(Debug, Serialize)]
struct AniListVariables<'a> {
    search: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
struct AniListResponseRaw {
    #[serde(rename = "data")]
    data: Option<AniListDataRaw>,
}

#[derive(Debug, Clone, Deserialize)]
struct AniListDataRaw {
    #[serde(rename = "Media")]
    media: Option<AniListMediaRaw>,
}

#[derive(Debug, Clone, Deserialize)]
struct AniListMediaRaw {
    title: AniListTitleRaw,
    #[serde(rename = "nextAiringEpisode")]
    next_airing_episode: Option<AniListAiringEpisodeRaw>,
    #[serde(rename = "seasonYear")]
    season_year: Option<i32>,
    #[serde(rename = "startDate")]
    start_date: AniListStartDateRaw,
    status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AniListTitleRaw {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AniListAiringEpisodeRaw {
    #[serde(rename = "airingAt")]
    airing_at: i64,
    episode: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
struct AniListStartDateRaw {
    year: Option<i32>,
    month: Option<u32>,
    day: Option<u32>,
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
    let mut terms = Vec::new();

    for candidate in [title.trim(), title_cn.trim()] {
        if candidate.is_empty() {
            continue;
        }

        push_unique_term(&mut terms, candidate);

        let trimmed = trim_variant(candidate);
        if !trimmed.is_empty() && trimmed != candidate {
            push_unique_term(&mut terms, &trimmed);
        }
    }

    terms
}

fn push_unique_term(terms: &mut Vec<String>, candidate: &str) {
    let normalized = normalize_title(candidate);
    if normalized.is_empty() {
        return;
    }

    if terms
        .iter()
        .any(|existing| normalize_title(existing) == normalized)
    {
        return;
    }

    terms.push(candidate.to_owned());
}

fn trim_variant(value: &str) -> String {
    let trimmed = value.trim();

    for marker in [" 第", " Season ", " season ", " Part ", " part "] {
        if let Some((head, _)) = trimmed.rsplit_once(marker) {
            let head = head.trim();
            if !head.is_empty() {
                return head.to_owned();
            }
        }
    }

    trimmed.to_owned()
}

fn select_broadcast_time(
    response: &AniListResponseRaw,
    title: &str,
    title_cn: &str,
    air_date: Option<&str>,
) -> Option<String> {
    let media = response.data.as_ref()?.media.as_ref()?;
    let airing = media.next_airing_episode.as_ref()?;
    let _episode_number = airing.episode;
    let _status = media.status.as_deref();
    let (air_year, air_month) = extract_year_month(air_date);

    if candidate_score(media, title, title_cn, air_year, air_month) < 48 {
        return None;
    }

    format_jst_time(airing.airing_at)
}

fn candidate_score(
    media: &AniListMediaRaw,
    title: &str,
    title_cn: &str,
    air_year: Option<i32>,
    air_month: Option<u32>,
) -> i32 {
    let primary = normalize_title(title);
    let secondary = normalize_title(title_cn);
    let mut score = 0;

    for candidate in [
        media.title.native.as_deref(),
        media.title.romaji.as_deref(),
        media.title.english.as_deref(),
    ] {
        let Some(candidate) = candidate else {
            continue;
        };

        let candidate = normalize_title(candidate);
        score = score.max(title_match_score(&candidate, &primary, 100, 52));
        score = score.max(title_match_score(&candidate, &secondary, 88, 44));
    }

    if air_year == media.season_year.or(media.start_date.year) {
        score += 18;
    }

    if air_month == media.start_date.month {
        score += 8;
    }

    if media.next_airing_episode.is_some() {
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

fn normalize_title(value: &str) -> String {
    let mut normalized = String::new();

    for character in value.chars() {
        if character.is_alphanumeric() {
            normalized.extend(character.to_lowercase());
        }
    }

    normalized
}
