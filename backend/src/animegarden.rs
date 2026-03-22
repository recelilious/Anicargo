use std::{collections::HashSet, time::Duration};

use anyhow::Context;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;
use tracing::{info, warn};

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
    pub season_hint: Option<i64>,
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
    pub parsed_episode_number: Option<f64>,
    pub parsed_episode_end_number: Option<f64>,
    pub parsed_season_number: Option<i64>,
    pub parsed_resolution: Option<String>,
    pub parsed_language: Option<String>,
    pub parsed_subtitles: Option<String>,
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
        info!(
            subject_id = profile.bangumi_subject_id,
            title = %profile.title,
            title_cn = %profile.title_cn,
            season_hint = ?profile.season_hint,
            "Starting AnimeGarden subject-first discovery"
        );
        let by_subject = self
            .fetch_resources(build_subject_params(profile.bangumi_subject_id))
            .await?;
        if !by_subject.is_empty() {
            info!(
                subject_id = profile.bangumi_subject_id,
                resource_count = by_subject.len(),
                "AnimeGarden subject search returned resources"
            );
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
                info!(
                    subject_id = profile.bangumi_subject_id,
                    keyword = %keyword,
                    resource_count = by_search.len(),
                    "AnimeGarden preferred keyword search returned resources"
                );
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
                info!(
                    subject_id = profile.bangumi_subject_id,
                    keyword = %keyword,
                    resource_count = by_search.len(),
                    "AnimeGarden secondary keyword search returned resources"
                );
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

    pub async fn search_episode_resources(
        &self,
        profile: &AnimeGardenSearchProfile,
        episode_number: f64,
    ) -> Result<AnimeGardenSearchResult, AppError> {
        let subject_episode_params =
            build_subject_episode_params(profile.bangumi_subject_id, episode_number);
        let subject_episode_results = self
            .fetch_resources_with_limits(subject_episode_params, 100, 1)
            .await?;
        info!(
            subject_id = profile.bangumi_subject_id,
            episode = episode_number,
            resource_count = subject_episode_results.len(),
            "AnimeGarden subject+keyword episode search finished"
        );
        if !subject_episode_results.is_empty() {
            return Ok(AnimeGardenSearchResult {
                strategy: format!(
                    "subject_keyword:{}",
                    format_episode_fragment(episode_number)
                ),
                resources: dedup_resources(subject_episode_results),
            });
        }

        let search_terms = build_episode_search_terms(profile, episode_number);
        if search_terms.is_empty() {
            return self.search_resources(profile).await;
        }

        info!(
            subject_id = profile.bangumi_subject_id,
            episode = episode_number,
            terms = ?search_terms,
            "Starting AnimeGarden targeted episode discovery"
        );

        for keyword in search_terms {
            let fetched = self
                .fetch_resources(build_search_params(keyword.clone()))
                .await?;
            info!(
                subject_id = profile.bangumi_subject_id,
                episode = episode_number,
                keyword = %keyword,
                resource_count = fetched.len(),
                "AnimeGarden targeted episode search finished"
            );
            if !fetched.is_empty() {
                return Ok(AnimeGardenSearchResult {
                    strategy: format!(
                        "episode:{}:{}",
                        format_episode_fragment(episode_number),
                        keyword
                    ),
                    resources: dedup_resources(fetched),
                });
            }
        }

        Ok(AnimeGardenSearchResult {
            strategy: format!("episode:{}:empty", format_episode_fragment(episode_number)),
            resources: Vec::new(),
        })
    }

    async fn fetch_resources(
        &self,
        extra_params: Vec<(String, String)>,
    ) -> Result<Vec<AnimeGardenResource>, AppError> {
        self.fetch_resources_with_limits(extra_params, self.page_size, self.max_pages)
            .await
    }

    async fn fetch_resources_with_limits(
        &self,
        extra_params: Vec<(String, String)>,
        page_size: usize,
        max_pages: usize,
    ) -> Result<Vec<AnimeGardenResource>, AppError> {
        let mut merged = Vec::new();
        const MAX_ATTEMPTS: usize = 4;

        for page in 1..=max_pages.max(1) {
            let mut query = vec![
                ("page".to_owned(), page.to_string()),
                ("pageSize".to_owned(), page_size.max(1).to_string()),
                ("metadata".to_owned(), "true".to_owned()),
            ];
            query.extend(extra_params.clone());

            let url = format!("{}/resources", self.base_url);
            let mut response = None;
            for attempt in 1..=MAX_ATTEMPTS {
                let request = self.http.get(&url).query(&query);
                match request.send().await {
                    Ok(result) => {
                        let status = result.status();
                        if status == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_ATTEMPTS {
                            warn!(
                                url = %url,
                                page,
                                attempt,
                                status = %status,
                                "AnimeGarden rate limited request; retrying with backoff"
                            );
                            sleep(Duration::from_millis((attempt as u64) * 1_500)).await;
                            continue;
                        }
                        response = Some(result);
                        break;
                    }
                    Err(error) if attempt < MAX_ATTEMPTS => {
                        warn!(
                            url = %url,
                            page,
                            attempt,
                            error = %error,
                            "Failed to reach AnimeGarden resources; retrying"
                        );
                        sleep(Duration::from_millis((attempt as u64) * 900)).await;
                    }
                    Err(error) => {
                        warn!(url = %url, page, error = %error, "Failed to reach AnimeGarden resources");
                        return Err(AppError::upstream("failed to reach AnimeGarden resources"));
                    }
                }
            }
            let Some(response) = response else {
                return Err(AppError::upstream("failed to reach AnimeGarden resources"));
            };

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

            sleep(Duration::from_millis(200)).await;
        }

        Ok(merged)
    }
}

fn build_subject_params(subject_id: i64) -> Vec<(String, String)> {
    vec![("subject".to_owned(), subject_id.to_string())]
}

fn build_subject_episode_params(subject_id: i64, episode_number: f64) -> Vec<(String, String)> {
    let mut params = build_subject_params(subject_id);
    params.push((
        "keyword".to_owned(),
        format_padded_episode_number(episode_number),
    ));
    params
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

fn build_episode_search_terms(
    profile: &AnimeGardenSearchProfile,
    episode_number: f64,
) -> Vec<String> {
    let mut terms = Vec::new();
    let padded_episode = if episode_number.fract().abs() < f64::EPSILON {
        format!("{:02}", episode_number.round() as i64)
    } else {
        format_episode_fragment(episode_number)
    };

    if let Some(season_hint) = profile.season_hint {
        let chinese_season = format_chinese_number(season_hint);
        if let Some(title) = sanitize_search_term(&profile.title) {
            push_term(
                &mut terms,
                Some(format!("{title} S{season_hint} {padded_episode}")),
            );
            push_term(
                &mut terms,
                Some(format!(
                    "{title} {} Season {padded_episode}",
                    format_english_ordinal(season_hint)
                )),
            );
        }
        if let Some(title_cn) = sanitize_search_term(&profile.title_cn) {
            push_term(
                &mut terms,
                Some(format!("{title_cn} 第{season_hint}季 {padded_episode}")),
            );
            if let Some(chinese_season) = chinese_season.as_deref() {
                push_term(
                    &mut terms,
                    Some(format!("{title_cn} 第{chinese_season}季 {padded_episode}")),
                );
                push_term(
                    &mut terms,
                    Some(format!("{title_cn} 第{chinese_season}期 {padded_episode}")),
                );
            }
        }
    }

    if let Some(title) = sanitize_search_term(&profile.title) {
        push_term(&mut terms, Some(format!("{title} {padded_episode}")));
    }
    if let Some(title_cn) = sanitize_search_term(&profile.title_cn) {
        push_term(&mut terms, Some(format!("{title_cn} {padded_episode}")));
    }

    terms
}

fn push_term(target: &mut Vec<String>, candidate: Option<String>) {
    let Some(candidate) = candidate else {
        return;
    };
    let normalized = candidate.trim();
    if normalized.is_empty() || target.iter().any(|value| value == normalized) {
        return;
    }
    target.push(normalized.to_owned());
}

fn format_episode_fragment(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}

fn format_padded_episode_number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{:02}", value.round() as i64)
    } else {
        format_episode_fragment(value)
    }
}

fn format_english_ordinal(value: i64) -> String {
    let suffix = match value % 100 {
        11..=13 => "th",
        _ => match value % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{value}{suffix}")
}

fn format_chinese_number(value: i64) -> Option<String> {
    match value {
        1 => Some("一".to_owned()),
        2 => Some("二".to_owned()),
        3 => Some("三".to_owned()),
        4 => Some("四".to_owned()),
        5 => Some("五".to_owned()),
        6 => Some("六".to_owned()),
        7 => Some("七".to_owned()),
        8 => Some("八".to_owned()),
        9 => Some("九".to_owned()),
        10 => Some("十".to_owned()),
        11..=19 => Some(format!("十{}", format_chinese_number(value - 10)?)),
        20..=99 => {
            let tens = value / 10;
            let ones = value % 10;
            if ones == 0 {
                Some(format!("{}十", format_chinese_number(tens)?))
            } else {
                Some(format!(
                    "{}十{}",
                    format_chinese_number(tens)?,
                    format_chinese_number(ones)?
                ))
            }
        }
        _ => None,
    }
}

fn dedup_resources(resources: Vec<AnimeGardenResource>) -> Vec<AnimeGardenResource> {
    let mut seen = HashSet::<(String, String)>::new();
    let mut deduped = Vec::new();
    for resource in resources {
        let key = (resource.provider.clone(), resource.provider_id.clone());
        if seen.insert(key) {
            deduped.push(resource);
        }
    }
    deduped
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
    #[serde(default)]
    metadata: Option<ResourceMetadataRaw>,
}

#[derive(Debug, Deserialize)]
struct FansubRaw {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PublisherRaw {
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct ResourceMetadataRaw {
    #[serde(default)]
    anipar: Option<AnimeGardenParseRaw>,
}

#[derive(Debug, Default, Deserialize)]
struct AnimeGardenParseRaw {
    #[serde(default)]
    episode: Option<AnimeGardenEpisodeRaw>,
    #[serde(default)]
    #[serde(rename = "episodeRange")]
    episode_range: Option<AnimeGardenEpisodeRangeRaw>,
    #[serde(default)]
    season: Option<AnimeGardenSeasonRaw>,
    #[serde(default)]
    file: Option<AnimeGardenFileRaw>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    subtitles: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct AnimeGardenEpisodeRaw {
    number: f64,
}

#[derive(Debug, Default, Deserialize)]
struct AnimeGardenEpisodeRangeRaw {
    from: f64,
    to: f64,
}

#[derive(Debug, Default, Deserialize)]
struct AnimeGardenSeasonRaw {
    number: i64,
}

#[derive(Debug, Default, Deserialize)]
struct AnimeGardenFileRaw {
    #[serde(default)]
    video: Option<AnimeGardenVideoRaw>,
}

#[derive(Debug, Default, Deserialize)]
struct AnimeGardenVideoRaw {
    #[serde(default)]
    resolution: Option<String>,
}

impl From<ResourceRaw> for AnimeGardenResource {
    fn from(value: ResourceRaw) -> Self {
        let parsed_episode_number = value
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.anipar.as_ref())
            .and_then(|parsed| parsed.episode.as_ref().map(|episode| episode.number))
            .or_else(|| {
                value
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.anipar.as_ref())
                    .and_then(|parsed| parsed.episode_range.as_ref().map(|range| range.from))
            });
        let parsed_episode_end_number = value
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.anipar.as_ref())
            .and_then(|parsed| parsed.episode_range.as_ref().map(|range| range.to))
            .or(parsed_episode_number);
        let parsed_season_number = value
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.anipar.as_ref())
            .and_then(|parsed| parsed.season.as_ref().map(|season| season.number));
        let parsed_resolution = value
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.anipar.as_ref())
            .and_then(|parsed| parsed.file.as_ref())
            .and_then(|file| file.video.as_ref())
            .and_then(|video| video.resolution.clone());
        let parsed_language = value
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.anipar.as_ref())
            .and_then(|parsed| parsed.language.clone());
        let parsed_subtitles = value
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.anipar.as_ref())
            .and_then(|parsed| parsed.subtitles.clone());

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
            parsed_episode_number,
            parsed_episode_end_number,
            parsed_season_number,
            parsed_resolution,
            parsed_language,
            parsed_subtitles,
        }
    }
}
