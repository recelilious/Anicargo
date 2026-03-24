use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use anyhow::Context;
use chrono::{DateTime, Datelike, FixedOffset, Utc};
use regex::Regex;
use reqwest::Client;

use crate::{
    config::YucConfig,
    types::{CatalogSectionDto, AppError, SubjectCardDto, SubjectDetailDto},
};

const CATALOG_PAGE_CACHE_TTL_HOURS: i64 = 12;

#[derive(Clone)]
pub struct YucClient {
    base_url: String,
    http: Client,
    page_cache: Arc<Mutex<HashMap<String, Vec<YucScheduleEntry>>>>,
    catalog_cache: Arc<Mutex<HashMap<String, CachedCatalogPage>>>,
}

#[derive(Clone)]
struct YucScheduleEntry {
    time: String,
    aliases: Vec<MatchTarget>,
}

#[derive(Clone)]
struct ExactScheduleEntry {
    title_cn: String,
    time: String,
    normalized: String,
    stripped: String,
}

#[derive(Clone)]
struct DetailScheduleEntry {
    title_cn: String,
    title_jp: String,
    normalized_cn: String,
    stripped_cn: String,
}

#[derive(Clone)]
struct MatchTarget {
    normalized: String,
    stripped: String,
}

#[derive(Clone)]
struct CachedCatalogPage {
    title: String,
    sections: Vec<CatalogSectionDto>,
    fetched_at: DateTime<Utc>,
}

impl YucClient {
    pub fn new(config: &YucConfig) -> anyhow::Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .context("failed to build yuc http client")?;

        Ok(Self {
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            http,
            page_cache: Arc::new(Mutex::new(HashMap::new())),
            catalog_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn current_season_key(&self) -> String {
        let now = beijing_now();
        let quarter_month = match now.month() {
            1..=3 => 1,
            4..=6 => 4,
            7..=9 => 7,
            _ => 10,
        };

        format!("{}{:02}", now.year(), quarter_month)
    }

    pub fn season_url(&self, season_key: &str) -> String {
        format!("{}/{}/", self.base_url, season_key)
    }

    pub fn preview_url(&self) -> String {
        format!("{}/new/", self.base_url)
    }

    pub fn special_url(&self) -> String {
        format!("{}/sp/", self.base_url)
    }

    pub async fn fetch_season_html(&self, season_key: &str) -> Result<String, AppError> {
        let url = self.season_url(season_key);
        self.fetch_html(&url, "Yuc season page").await
    }

    pub async fn fetch_preview_catalog(&self) -> Result<(String, Vec<CatalogSectionDto>), AppError> {
        self.fetch_cached_catalog_page("preview", "新季度前瞻", &self.preview_url(), parse_preview_sections)
            .await
    }

    pub async fn fetch_special_catalog(&self) -> Result<(String, Vec<CatalogSectionDto>), AppError> {
        self.fetch_cached_catalog_page("special", "特别放送", &self.special_url(), parse_special_sections)
            .await
    }

    pub async fn has_preview_catalog(&self) -> Result<bool, AppError> {
        let (_, sections) = self.fetch_preview_catalog().await?;
        Ok(sections.iter().any(|section| !section.items.is_empty()))
    }

    async fn fetch_cached_catalog_page(
        &self,
        cache_key: &str,
        fallback_title: &str,
        url: &str,
        parser: fn(&str) -> Vec<CatalogSectionDto>,
    ) -> Result<(String, Vec<CatalogSectionDto>), AppError> {
        if let Some(cached) = self
            .catalog_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(cache_key).cloned())
            .filter(|cached| Utc::now() - cached.fetched_at < chrono::Duration::hours(CATALOG_PAGE_CACHE_TTL_HOURS))
        {
            return Ok((cached.title, cached.sections));
        }

        let html = self.fetch_html(url, "Yuc catalog page").await?;
        let title = extract_page_title(&html).unwrap_or_else(|| fallback_title.to_owned());
        let sections = parser(&html);
        if let Ok(mut cache) = self.catalog_cache.lock() {
            cache.insert(
                cache_key.to_owned(),
                CachedCatalogPage {
                    title: title.clone(),
                    sections: sections.clone(),
                    fetched_at: Utc::now(),
                },
            );
        }

        Ok((title, sections))
    }

    async fn fetch_html(&self, url: &str, label: &str) -> Result<String, AppError> {
        self.http
            .get(url)
            .send()
            .await
            .map_err(|error| {
                AppError::upstream(format!("failed to reach {label}: {error}"))
            })?
            .error_for_status()
            .map_err(|error| {
                AppError::upstream(format!("{label} returned an error: {error}"))
            })?
            .text()
            .await
            .map_err(|error| AppError::upstream(format!("failed to read {label}: {error}")))
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
        let season_key = season_key_from_air_date(air_date)?;
        let entries = self.load_entries(&season_key).await.ok()?;

        select_broadcast_time(&entries, title, title_cn)
    }

    async fn load_entries(&self, season_key: &str) -> anyhow::Result<Vec<YucScheduleEntry>> {
        if let Some(cached) = self
            .page_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(season_key).cloned())
        {
            return Ok(cached);
        }

        let url = format!("{}/{}/", self.base_url, season_key);
        let html = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("failed to reach Yuc season page '{}'", url))?
            .error_for_status()
            .with_context(|| format!("Yuc returned an error for season page '{}'", url))?
            .text()
            .await
            .with_context(|| format!("failed to read Yuc season page '{}'", url))?;

        let entries = parse_schedule_entries(&html);

        if let Ok(mut cache) = self.page_cache.lock() {
            cache.insert(season_key.to_owned(), entries.clone());
        }

        Ok(entries)
    }
}

fn parse_schedule_entries(html: &str) -> Vec<YucScheduleEntry> {
    let exact_entries = parse_exact_entries(html);
    if exact_entries.is_empty() {
        return Vec::new();
    }

    let detail_entries = parse_detail_entries(html);
    merge_entries(exact_entries, detail_entries)
}

fn parse_exact_entries(html: &str) -> Vec<ExactScheduleEntry> {
    let mut entries = Vec::new();

    for capture in schedule_card_regex().captures_iter(html) {
        let Some(time) = capture.name("time").map(|value| value.as_str().trim()) else {
            continue;
        };
        let Some(raw_title) = capture.name("title").map(|value| value.as_str()) else {
            continue;
        };

        let title = sanitize_title(raw_title);
        if title.is_empty() {
            continue;
        }

        entries.push(ExactScheduleEntry {
            normalized: normalize_title(&title),
            stripped: strip_variant(&title),
            title_cn: title,
            time: time.to_owned(),
        });
    }

    entries
}

fn parse_detail_entries(html: &str) -> Vec<DetailScheduleEntry> {
    let mut entries = Vec::new();

    for capture in detail_card_regex().captures_iter(html) {
        let title_cn = capture
            .name("title_cn")
            .map(|value| sanitize_title(value.as_str()))
            .unwrap_or_default();
        let title_jp = capture
            .name("title_jp")
            .map(|value| sanitize_title(value.as_str()))
            .unwrap_or_default();

        if title_cn.is_empty() {
            continue;
        }

        entries.push(DetailScheduleEntry {
            normalized_cn: normalize_title(&title_cn),
            stripped_cn: strip_variant(&title_cn),
            title_cn,
            title_jp,
        });
    }

    entries
}

fn merge_entries(
    exact_entries: Vec<ExactScheduleEntry>,
    detail_entries: Vec<DetailScheduleEntry>,
) -> Vec<YucScheduleEntry> {
    let mut merged = exact_entries
        .iter()
        .map(|entry| YucScheduleEntry {
            time: entry.time.clone(),
            aliases: vec![build_match_target(&entry.title_cn)],
        })
        .collect::<Vec<_>>();

    for detail in detail_entries {
        let Some((best_index, best_score)) = exact_entries
            .iter()
            .enumerate()
            .map(|(index, exact)| {
                (
                    index,
                    score_text_pair(
                        &exact.normalized,
                        &exact.stripped,
                        &detail.normalized_cn,
                        &detail.stripped_cn,
                    ),
                )
            })
            .max_by_key(|(_, score)| *score)
        else {
            continue;
        };

        if best_score < 72 {
            continue;
        }

        push_alias(&mut merged[best_index], &detail.title_cn);
        push_alias(&mut merged[best_index], &detail.title_jp);
    }

    merged
}

fn select_broadcast_time(
    entries: &[YucScheduleEntry],
    title: &str,
    title_cn: &str,
) -> Option<String> {
    let targets = build_match_targets(title, title_cn);
    if targets.is_empty() {
        return None;
    }

    let mut scored = entries
        .iter()
        .map(|entry| (score_entry(entry, &targets), entry))
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| right.0.cmp(&left.0));

    let (best_score, best_entry) = scored.first()?;
    let runner_up = scored.get(1).map(|(score, _)| *score).unwrap_or_default();

    if *best_score < 56 {
        return None;
    }

    if *best_score < 100 && (*best_score - runner_up) < 7 {
        return None;
    }

    Some(best_entry.time.clone())
}

fn score_entry(entry: &YucScheduleEntry, targets: &[MatchTarget]) -> i32 {
    entry
        .aliases
        .iter()
        .flat_map(|alias| {
            targets.iter().map(move |target| {
                score_text_pair(
                    &alias.normalized,
                    &alias.stripped,
                    &target.normalized,
                    &target.stripped,
                )
            })
        })
        .max()
        .unwrap_or_default()
}

fn score_text_pair(
    left_normalized: &str,
    left_stripped: &str,
    right_normalized: &str,
    right_stripped: &str,
) -> i32 {
    let mut score = 0;

    if !left_normalized.is_empty() && left_normalized == right_normalized {
        score = score.max(140);
    }

    if !left_stripped.is_empty() && left_stripped == right_stripped {
        score = score.max(136);
    }

    if !left_stripped.is_empty()
        && !right_stripped.is_empty()
        && (left_normalized.contains(right_stripped)
            || right_normalized.contains(left_stripped)
            || left_stripped.contains(right_stripped)
            || right_stripped.contains(left_stripped))
    {
        score = score.max(108);
    }

    score = score.max((dice_coefficient(left_normalized, right_normalized) * 100.0).round() as i32);
    score = score.max((dice_coefficient(left_stripped, right_stripped) * 112.0).round() as i32);

    score
}

fn build_match_targets(title: &str, title_cn: &str) -> Vec<MatchTarget> {
    let mut targets = Vec::new();

    for candidate in [title_cn.trim(), title.trim()] {
        let Some(target) = build_optional_match_target(candidate) else {
            continue;
        };

        if targets.iter().any(|existing: &MatchTarget| {
            existing.normalized == target.normalized && existing.stripped == target.stripped
        }) {
            continue;
        }

        targets.push(target);
    }

    targets
}

fn push_alias(entry: &mut YucScheduleEntry, alias: &str) {
    let Some(target) = build_optional_match_target(alias.trim()) else {
        return;
    };

    if entry.aliases.iter().any(|existing| {
        existing.normalized == target.normalized && existing.stripped == target.stripped
    }) {
        return;
    }

    entry.aliases.push(target);
}

fn build_match_target(value: &str) -> MatchTarget {
    MatchTarget {
        normalized: normalize_title(value),
        stripped: strip_variant(value),
    }
}

fn build_optional_match_target(value: &str) -> Option<MatchTarget> {
    if value.is_empty() {
        return None;
    }

    let target = build_match_target(value);
    if target.normalized.is_empty() {
        return None;
    }

    Some(target)
}

fn season_key_from_air_date(air_date: Option<&str>) -> Option<String> {
    let value = air_date?;
    let date_part = value.split_once('T').map(|(left, _)| left).unwrap_or(value);
    let mut segments = date_part.split('-');
    let year = segments.next()?.parse::<i32>().ok()?;
    let month = segments.next()?.parse::<u32>().ok()?;

    let quarter_month = match month {
        1..=3 => 1,
        4..=6 => 4,
        7..=9 => 7,
        _ => 10,
    };

    Some(format!("{year}{quarter_month:02}"))
}

fn parse_preview_sections(html: &str) -> Vec<CatalogSectionDto> {
    let mut sections = Vec::new();

    for (section_index, capture) in preview_section_regex().captures_iter(html).enumerate() {
        let Some(title) = capture
            .name("title")
            .map(|value| sanitize_title(value.as_str()))
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(content) = capture.name("content").map(|value| value.as_str()) else {
            continue;
        };

        let items = preview_card_regex()
            .captures_iter(content)
            .filter_map(|card| {
                let title = card
                    .name("title")
                    .map(|value| sanitize_title(value.as_str()))
                    .unwrap_or_default();
                if title.is_empty() {
                    return None;
                }

                let media_type = card
                    .name("kind")
                    .map(|value| sanitize_title(value.as_str()))
                    .filter(|value| !value.is_empty());
                let date_label = card
                    .name("date")
                    .map(|value| sanitize_title(value.as_str()))
                    .filter(|value| !value.is_empty());
                let catalog_label = join_catalog_label(media_type.as_deref(), date_label.as_deref());

                Some(build_catalog_card(
                    stable_catalog_subject_id("preview", &title),
                    &title,
                    card.name("image").map(|value| value.as_str().trim().to_owned()),
                    catalog_label,
                    "upcoming",
                ))
            })
            .collect::<Vec<_>>();

        if items.is_empty() {
            continue;
        }

        sections.push(CatalogSectionDto {
            key: format!("preview-{}", section_index + 1),
            title,
            items,
        });
    }

    sections
}

fn parse_special_sections(html: &str) -> Vec<CatalogSectionDto> {
    let mut sections = Vec::new();

    for (section_index, capture) in special_section_regex().captures_iter(html).enumerate() {
        let Some(title) = capture
            .name("title")
            .map(|value| sanitize_title(value.as_str()))
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(content) = capture.name("content").map(|value| value.as_str()) else {
            continue;
        };

        let items = special_card_regex()
            .captures_iter(content)
            .filter_map(|card| {
                let title = card
                    .name("title")
                    .map(|value| sanitize_title(value.as_str()))
                    .unwrap_or_default();
                if title.is_empty() {
                    return None;
                }

                let media_type = card
                    .name("kind")
                    .map(|value| sanitize_title(value.as_str()))
                    .filter(|value| !value.is_empty());
                let release = card
                    .name("release")
                    .map(|value| sanitize_title(value.as_str()))
                    .filter(|value| !value.is_empty());
                let catalog_label = join_catalog_label(media_type.as_deref(), release.as_deref());
                let release_status = special_release_status(release.as_deref());

                Some(build_catalog_card(
                    stable_catalog_subject_id("special", &title),
                    &title,
                    card.name("image").map(|value| value.as_str().trim().to_owned()),
                    catalog_label,
                    release_status,
                ))
            })
            .collect::<Vec<_>>();

        if items.is_empty() {
            continue;
        }

        sections.push(CatalogSectionDto {
            key: format!("special-{}", section_index + 1),
            title,
            items,
        });
    }

    sections
}

fn join_catalog_label(left: Option<&str>, right: Option<&str>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(format!("{left} {right}")),
        (Some(left), None) => Some(left.to_owned()),
        (None, Some(right)) => Some(right.to_owned()),
        (None, None) => None,
    }
}

fn build_catalog_card(
    bangumi_subject_id: i64,
    title: &str,
    image_portrait: Option<String>,
    catalog_label: Option<String>,
    release_status: &str,
) -> SubjectCardDto {
    SubjectCardDto {
        bangumi_subject_id,
        title: title.to_owned(),
        title_cn: title.to_owned(),
        summary: String::new(),
        release_status: release_status.to_owned(),
        air_date: None,
        broadcast_time: None,
        air_weekday: None,
        image_banner: image_portrait.clone(),
        image_portrait,
        tags: Vec::new(),
        total_episodes: None,
        rating_score: None,
        catalog_label,
    }
}

fn stable_catalog_subject_id(prefix: &str, title: &str) -> i64 {
    let mut normalized = String::with_capacity(prefix.len() + title.len() + 1);
    normalized.push_str(prefix);
    normalized.push(':');
    normalized.push_str(title);
    let hash = normalize_title(&normalized)
        .chars()
        .fold(0i64, |acc, character| acc.wrapping_mul(131).wrapping_add(character as i64));
    -hash.abs().max(1)
}

fn special_release_status(release: Option<&str>) -> &'static str {
    let Some(release) = release else {
        return "completed";
    };

    let Some((year, month, day)) = extract_release_date(release) else {
        return if release.contains("2026") || release.contains("2027") {
            "upcoming"
        } else {
            "completed"
        };
    };

    let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month, day) else {
        return "completed";
    };

    if date >= beijing_now().date_naive() {
        "upcoming"
    } else {
        "completed"
    }
}

fn extract_release_date(release: &str) -> Option<(i32, u32, u32)> {
    let capture = release_date_regex().captures(release)?;
    let year = capture.name("year")?.as_str().parse::<i32>().ok()?;
    let month = capture.name("month")?.as_str().parse::<u32>().ok()?;
    let day = capture.name("day")?.as_str().parse::<u32>().ok()?;
    Some((year, month, day))
}

fn extract_page_title(html: &str) -> Option<String> {
    let capture = page_title_regex().captures(html)?;
    let value = capture.name("title")?.as_str();
    let cleaned = sanitize_title(value);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn beijing_now() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).expect("valid beijing utc offset"))
}

fn sanitize_title(raw: &str) -> String {
    let without_tags = html_tag_regex().replace_all(raw, " ");
    without_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&ldquo;", "\"")
        .replace("&rdquo;", "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_variant(value: &str) -> String {
    let stripped = variant_regex().replace_all(value, "");
    normalize_title(&stripped)
}

fn normalize_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn dice_coefficient(left: &str, right: &str) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    if left == right {
        return 1.0;
    }

    let left_pairs = bigrams(left);
    let right_pairs = bigrams(right);

    if left_pairs.is_empty() || right_pairs.is_empty() {
        return 0.0;
    }

    let mut overlap = 0usize;
    let mut counts = HashMap::new();
    for pair in &left_pairs {
        *counts.entry(pair.clone()).or_insert(0usize) += 1;
    }

    for pair in &right_pairs {
        if let Some(count) = counts.get_mut(pair) {
            if *count > 0 {
                *count -= 1;
                overlap += 1;
            }
        }
    }

    (2 * overlap) as f32 / (left_pairs.len() + right_pairs.len()) as f32
}

fn bigrams(value: &str) -> Vec<String> {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() < 2 {
        return Vec::new();
    }

    chars
        .windows(2)
        .map(|window| window.iter().collect::<String>())
        .collect()
}

fn schedule_card_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?s)<div style="float:left"><div class="div_date"><p class="imgtext\d+">(?P<time>\d{2}:\d{2})~</p><p class="imgep">.*?</p><img[^>]*></div><div><table width="120px"><tr><td colspan="3" class="date_title_[^"]*">(?P<title>.*?)</td></tr>"#,
        )
        .expect("valid yuc schedule regex")
    })
}

fn preview_section_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?s)<p class="future_intro"><b>(?P<title>.*?)</b></p>\s*(?P<content>.*?)<div style="clear:both"></div>"#,
        )
        .expect("valid yuc preview section regex")
    })
}

fn preview_card_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?s)<div style="float:left"><div class="future_div">\s*<p class="future_type[^"]*">(?P<kind>.*?)</p><p class="future_date[^"]*">(?P<date>.*?)</p>\s*<img width="150px" data-src="(?P<image>[^"]*)"[^>]*></div><div><table class="future_table"><tr class="future_tr">\s*<td class="future_title[^"]*">(?P<title>.*?)</td>"#,
        )
        .expect("valid yuc preview card regex")
    })
}

fn special_section_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?s)<details><summary><p class="intro_sp"><b>(?P<title>.*?)</b></p></summary>\s*<div>\s*(?P<content>.*?)</div><div style="clear:both"></div></details>"#,
        )
        .expect("valid yuc special section regex")
    })
}

fn special_card_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?s)<div style="float:left"><div class="div_sp"[^>]*><img[^>]*data-src="(?P<image>[^"]*)"[^>]*></div><div><table width="200px"><tr><td colspan="2" class="sp_title[^"]*">(?P<title>.*?)</td></tr><tr><td class="type-[^"]*">(?P<kind>.*?)</td><td class="sp_release">(?P<release>.*?)</td></tr></table></div></div>"#,
        )
        .expect("valid yuc special card regex")
    })
}

fn release_date_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?P<year>\d{4})[年/](?P<month>\d{1,2})[月/](?P<day>\d{1,2})"#)
            .expect("valid release date regex")
    })
}

fn page_title_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?s)<title>(?P<title>.*?)</title>"#).expect("valid page title regex")
    })
}

fn detail_card_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?s)<p class="title_cn_[^"]*">(?P<title_cn>.*?)</p>\s*<p class="title_jp_[^"]*">(?P<title_jp>.*?)</p>.*?<p class="broadcast_r">.*?</p>"#,
        )
        .expect("valid yuc detail regex")
    })
}

fn html_tag_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"<[^>]+>").expect("valid html tag regex"))
}

fn variant_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(第\s*[0-9一二三四五六七八九十百零两]+\s*(?:季|期|部|篇|章|幕)|[Pp]art\.?\s*[0-9]+|[Ss]eason\s*[0-9]+|最[终終]章|最[终終]期|最[终終]幕|完[结結]篇)",
        )
        .expect("valid title variant regex")
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_preview_sections, parse_special_sections};

    #[test]
    fn parses_preview_sections_into_catalog_cards() {
        let html = r#"
            <title>新番卫星观测站 | 長門番堂</title>
            <p class="future_intro"><b>预计着陆 [夏季档] (2099年7月±)</b></p>
            <div style="float:left"><div class="future_div">
                <p class="future_type_a">原创</p><p class="future_date_">2099夏</p>
                <img width="150px" data-src="https://example.com/a.jpg" referrerPolicy="no-referrer"></div><div><table class="future_table"><tr class="future_tr">
                <td class="future_title__">测试前瞻作品</td></tr></table></div></div>
            <div style="clear:both"></div>
        "#;

        let sections = parse_preview_sections(html);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "预计着陆 [夏季档] (2099年7月±)");
        assert_eq!(sections[0].items.len(), 1);
        assert_eq!(sections[0].items[0].title_cn, "测试前瞻作品");
        assert_eq!(sections[0].items[0].catalog_label.as_deref(), Some("原创 2099夏"));
        assert_eq!(sections[0].items[0].release_status, "upcoming");
    }

    #[test]
    fn parses_special_sections_into_catalog_cards() {
        let html = r#"
            <title>Movie/OVA/OAD/SP etc. | 長門番堂</title>
            <details><summary><p class="intro_sp"><b>-----2099年3月-----</b></p></summary>
            <div>
            <div style="float:left"><div class="div_sp" ><img width="200px" data-src="https://example.com/b.jpg" referrerPolicy="no-referrer"></div><div><table width="200px"><tr><td colspan="2" class="sp_title">测试剧场版</td></tr><tr><td class="type-m">Movie</td><td class="sp_release">2099/3/6上映</td></tr></table></div></div>
            </div><div style="clear:both"></div></details>
        "#;

        let sections = parse_special_sections(html);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "-----2099年3月-----");
        assert_eq!(sections[0].items.len(), 1);
        assert_eq!(sections[0].items[0].title_cn, "测试剧场版");
        assert_eq!(sections[0].items[0].catalog_label.as_deref(), Some("Movie 2099/3/6上映"));
        assert_eq!(sections[0].items[0].release_status, "upcoming");
    }
}
