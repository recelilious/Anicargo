use std::{collections::HashMap, sync::OnceLock};

use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use sqlx::SqlitePool;

use crate::{
    animegarden::{AnimeGardenClient, AnimeGardenResource, AnimeGardenSearchProfile},
    db::{self, NewResourceCandidate},
    media::{ParsedReleaseSlot, infer_release_slot},
    types::{AppError, DownloadJobDto, FansubRuleDto, PolicyDto, ResourceCandidateDto},
};

#[derive(Clone)]
pub struct ResourceDiscoveryCoordinator {
    animegarden: AnimeGardenClient,
}

impl ResourceDiscoveryCoordinator {
    pub fn new(animegarden: AnimeGardenClient) -> Self {
        Self { animegarden }
    }

    pub async fn discover_for_job(
        &self,
        pool: &SqlitePool,
        job: &DownloadJobDto,
        profile: &AnimeGardenSearchProfile,
        policy: &PolicyDto,
        episode_targets: Option<&[f64]>,
    ) -> Result<Vec<ResourceCandidateDto>, AppError> {
        let (strategy, resources) = if let Some(targets) =
            episode_targets.filter(|targets| !targets.is_empty())
        {
            let mut strategy_parts = Vec::new();
            let mut normalized_resources = Vec::new();

            for target_episode in targets {
                let search = self
                    .animegarden
                    .search_episode_resources(profile, *target_episode)
                    .await?;
                strategy_parts.push(search.strategy);
                normalized_resources.extend(
                    normalize_resource_release_slots(
                        search.resources,
                        profile,
                        &job.release_status,
                    )
                    .into_iter()
                    .filter(|resource| {
                        release_slot_matches_target_episode(&resource.release_slot, *target_episode)
                    }),
                );
            }

            (
                format!("targeted:{}", strategy_parts.join(" || ")),
                normalized_resources,
            )
        } else {
            let search = self.animegarden.search_resources(profile).await?;
            (
                search.strategy,
                normalize_resource_release_slots(search.resources, profile, &job.release_status),
            )
        };
        let search_run_id =
            db::start_resource_search_run(pool, job.id, job.bangumi_subject_id, &strategy).await?;

        let rules = db::list_fansub_rules(pool).await?;
        let previous_selected =
            db::latest_selected_candidate_for_subject(pool, job.bangumi_subject_id).await?;
        let current_selected = db::current_selected_candidate_for_job(pool, job.id).await?;

        let mut stored = Vec::new();
        for resource in resources {
            let evaluation = evaluate_candidate(
                &resource.resource,
                &rules,
                previous_selected.as_ref(),
                policy,
                &job.release_status,
            );
            let candidate = db::create_resource_candidate(
                pool,
                NewResourceCandidate {
                    download_job_id: job.id,
                    search_run_id,
                    bangumi_subject_id: job.bangumi_subject_id,
                    provider: resource.resource.provider,
                    provider_resource_id: resource.resource.provider_id,
                    title: resource.resource.title,
                    href: resource.resource.href,
                    magnet: resource.resource.magnet,
                    release_type: resource.resource.release_type,
                    size_bytes: resource.resource.size,
                    fansub_name: resource.resource.fansub_name,
                    publisher_name: resource.resource.publisher_name,
                    slot_key: resource.release_slot.slot_key,
                    episode_index: resource.release_slot.episode_index,
                    episode_end_index: resource.release_slot.episode_end_index,
                    is_collection: resource.release_slot.is_collection,
                    source_created_at: resource.resource.created_at,
                    source_fetched_at: resource.resource.fetched_at,
                    resolution: evaluation.resolution,
                    locale_hint: evaluation.locale_hint,
                    is_raw: evaluation.is_raw,
                    score: evaluation.score,
                    rejected_reason: evaluation.rejected_reason,
                },
            )
            .await?;
            stored.push(candidate);
        }

        let (selected_candidate_id, status, notes) =
            choose_candidate(job, current_selected.as_ref(), &stored, policy);
        if current_selected.map(|candidate| candidate.id) != selected_candidate_id {
            db::assign_download_job_candidate(pool, job.id, selected_candidate_id).await?;
        }
        db::finish_resource_search_run(
            pool,
            search_run_id,
            job.id,
            status,
            stored.len() as i64,
            selected_candidate_id,
            Some(&notes),
        )
        .await?;

        db::list_resource_candidates(pool, job.id).await
    }
}

fn release_slot_matches_target_episode(slot: &ParsedReleaseSlot, target_episode: f64) -> bool {
    let Some(start) = slot.episode_index else {
        return false;
    };
    let end = slot.episode_end_index.unwrap_or(start);
    target_episode + 0.001 >= start && target_episode - 0.001 <= end
}

#[derive(Debug, Clone)]
struct NormalizedAnimeGardenResource {
    resource: AnimeGardenResource,
    release_slot: ParsedReleaseSlot,
}

#[derive(Debug, Clone)]
struct EpisodeObservation {
    index: usize,
    raw_slot: ParsedReleaseSlot,
    created_at: Option<DateTime<Utc>>,
    has_explicit_current_season_marker: bool,
}

fn normalize_resource_release_slots(
    resources: Vec<AnimeGardenResource>,
    profile: &AnimeGardenSearchProfile,
    release_status: &str,
) -> Vec<NormalizedAnimeGardenResource> {
    let canonical_season_hint = profile
        .season_hint
        .or_else(|| most_common_resource_season_number(&resources));
    let observations = resources
        .iter()
        .enumerate()
        .map(|(index, resource)| {
            let raw_slot = infer_raw_release_slot(resource, release_status);
            let title_season_hint = infer_season_hint_from_texts([resource.title.as_str()]);
            EpisodeObservation {
                index,
                created_at: parse_resource_timestamp(&resource.created_at),
                has_explicit_current_season_marker: canonical_season_hint.is_some_and(|season| {
                    resource.parsed_season_number == Some(season)
                        || title_season_hint == Some(season)
                }),
                raw_slot,
            }
        })
        .collect::<Vec<_>>();
    let inferred_offset = infer_episode_offset(&observations, canonical_season_hint);
    let max_relative_episode = observations
        .iter()
        .filter(|item| {
            item.has_explicit_current_season_marker
                && !item.raw_slot.is_collection
                && item.raw_slot.episode_index.is_some()
        })
        .filter_map(|item| {
            item.raw_slot
                .episode_end_index
                .or(item.raw_slot.episode_index)
        })
        .max_by(|left, right| left.total_cmp(right));

    observations
        .into_iter()
        .map(|observation| {
            let mut release_slot = observation.raw_slot.clone();
            if should_normalize_observation(
                &observation,
                inferred_offset,
                max_relative_episode,
                canonical_season_hint,
            ) {
                release_slot = normalize_slot_with_offset(&observation.raw_slot, inferred_offset);
            }

            NormalizedAnimeGardenResource {
                resource: resources[observation.index].clone(),
                release_slot,
            }
        })
        .collect()
}

fn most_common_resource_season_number(resources: &[AnimeGardenResource]) -> Option<i64> {
    let mut counts = HashMap::<i64, usize>::new();
    for season_number in resources
        .iter()
        .filter_map(|resource| resource.parsed_season_number)
    {
        *counts.entry(season_number).or_default() += 1;
    }

    counts
        .into_iter()
        .max_by_key(|(season_number, count)| (*count, *season_number))
        .map(|(season_number, _)| season_number)
}

fn infer_raw_release_slot(
    resource: &AnimeGardenResource,
    release_status: &str,
) -> ParsedReleaseSlot {
    if let Some(episode) = resource.parsed_episode_number {
        let end = resource.parsed_episode_end_number.unwrap_or(episode);
        if end > episode {
            return ParsedReleaseSlot {
                slot_key: format!(
                    "batch:{}-{}",
                    format_episode_fragment(episode),
                    format_episode_fragment(end)
                ),
                episode_index: Some(episode),
                episode_end_index: Some(end),
                is_collection: true,
            };
        }

        return ParsedReleaseSlot {
            slot_key: format!("episode:{}", format_episode_fragment(episode)),
            episode_index: Some(episode),
            episode_end_index: Some(episode),
            is_collection: false,
        };
    }

    infer_release_slot(
        &resource.title,
        &resource.release_type,
        &resource.provider_id,
        release_status,
    )
}

fn parse_resource_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

fn infer_episode_offset(
    observations: &[EpisodeObservation],
    canonical_season_hint: Option<i64>,
) -> Option<f64> {
    let relative = observations
        .iter()
        .filter(|item| {
            item.has_explicit_current_season_marker
                && !item.raw_slot.is_collection
                && item.raw_slot.episode_index.is_some()
        })
        .collect::<Vec<_>>();
    let absolute = observations
        .iter()
        .filter(|item| {
            !item.raw_slot.is_collection
                && item.raw_slot.episode_index.is_some()
                && !item.has_explicit_current_season_marker
                && canonical_season_hint.is_some()
        })
        .collect::<Vec<_>>();

    if relative.is_empty() || absolute.is_empty() {
        return None;
    }

    let mut counts = HashMap::<i64, usize>::new();
    for candidate in absolute {
        let Some(candidate_episode) = candidate.raw_slot.episode_index else {
            continue;
        };
        let Some(candidate_created_at) = candidate.created_at else {
            continue;
        };

        let nearest = relative
            .iter()
            .filter_map(|relative_item| {
                let relative_episode = relative_item.raw_slot.episode_index?;
                let relative_created_at = relative_item.created_at?;
                let delta = (candidate_created_at - relative_created_at)
                    .num_hours()
                    .abs();
                (delta <= 72).then_some((relative_episode, delta))
            })
            .min_by_key(|(_, delta)| *delta);

        let Some((relative_episode, _)) = nearest else {
            continue;
        };

        let offset = candidate_episode - relative_episode;
        if offset >= 6.0 && offset.fract().abs() < f64::EPSILON {
            *counts.entry(offset.round() as i64).or_default() += 1;
        }
    }

    let Some((offset, count)) = counts
        .into_iter()
        .max_by_key(|(offset, count)| (*count, *offset))
    else {
        return None;
    };

    if count >= 2 || offset >= 12 {
        Some(offset as f64)
    } else {
        None
    }
}

fn should_normalize_observation(
    observation: &EpisodeObservation,
    inferred_offset: Option<f64>,
    max_relative_episode: Option<f64>,
    canonical_season_hint: Option<i64>,
) -> bool {
    let Some(offset) = inferred_offset else {
        return false;
    };
    if observation.has_explicit_current_season_marker || observation.raw_slot.is_collection {
        return false;
    }
    if canonical_season_hint.is_none() {
        return false;
    }
    let Some(raw_episode) = observation.raw_slot.episode_index else {
        return false;
    };

    let threshold = max_relative_episode.unwrap_or(0.0) + 1.0;
    raw_episode > threshold && raw_episode - offset > 0.0
}

fn normalize_slot_with_offset(slot: &ParsedReleaseSlot, offset: Option<f64>) -> ParsedReleaseSlot {
    let Some(offset) = offset else {
        return slot.clone();
    };

    let Some(start) = slot.episode_index else {
        return slot.clone();
    };
    let end = slot.episode_end_index.unwrap_or(start);
    let normalized_start = start - offset;
    let normalized_end = end - offset;
    if normalized_start <= 0.0 || normalized_end < normalized_start {
        return slot.clone();
    }

    if slot.is_collection {
        ParsedReleaseSlot {
            slot_key: format!(
                "batch:{}-{}",
                format_episode_fragment(normalized_start),
                format_episode_fragment(normalized_end)
            ),
            episode_index: Some(normalized_start),
            episode_end_index: Some(normalized_end),
            is_collection: true,
        }
    } else {
        ParsedReleaseSlot {
            slot_key: format!("episode:{}", format_episode_fragment(normalized_start)),
            episode_index: Some(normalized_start),
            episode_end_index: Some(normalized_start),
            is_collection: false,
        }
    }
}

fn format_episode_fragment(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}

struct CandidateEvaluation {
    score: f64,
    resolution: Option<String>,
    locale_hint: Option<String>,
    is_raw: bool,
    rejected_reason: Option<String>,
}

fn choose_candidate(
    job: &DownloadJobDto,
    current_selected: Option<&ResourceCandidateDto>,
    candidates: &[ResourceCandidateDto],
    policy: &PolicyDto,
) -> (Option<i64>, &'static str, String) {
    let best = candidates
        .iter()
        .filter(|candidate| candidate.rejected_reason.is_none())
        .max_by(|left, right| {
            candidate_priority_key(left, &job.release_status)
                .cmp(&candidate_priority_key(right, &job.release_status))
        });

    let Some(best) = best else {
        if let Some(current) = current_selected {
            return (
                Some(current.id),
                "retained",
                "No new acceptable candidates; retained current selection".to_owned(),
            );
        }

        return (
            None,
            "empty",
            "No acceptable resource candidates found".to_owned(),
        );
    };

    if let Some(current) = current_selected {
        if current.slot_key == best.slot_key {
            if !within_replacement_window(
                job.selection_updated_at.as_deref(),
                policy.replacement_window_hours,
            ) {
                return (
                    Some(current.id),
                    "retained",
                    "Replacement window expired; retained existing selected candidate".to_owned(),
                );
            }

            if candidate_priority_key(best, &job.release_status)
                <= candidate_priority_key(current, &job.release_status)
            {
                return (
                    Some(current.id),
                    "retained",
                    "Current selected candidate remains preferred within replacement window"
                        .to_owned(),
                );
            }

            return (
                Some(best.id),
                "selected",
                "Selected a better candidate for the same episode slot".to_owned(),
            );
        }

        return (
            Some(best.id),
            "selected",
            "Selected a newer resource slot without replacing the previous one".to_owned(),
        );
    }

    (
        Some(best.id),
        "selected",
        "Selected best available resource candidate".to_owned(),
    )
}

pub(crate) fn candidate_priority_key(
    candidate: &ResourceCandidateDto,
    release_status: &str,
) -> (i64, i64, i64, i64) {
    let slot_weight = match release_status {
        "airing" | "upcoming" => {
            if candidate.is_collection {
                -1
            } else {
                candidate
                    .episode_end_index
                    .or(candidate.episode_index)
                    .map(|value| (value * 100.0).round() as i64)
                    .unwrap_or(0)
            }
        }
        _ => {
            if candidate.is_collection {
                candidate
                    .episode_end_index
                    .or(candidate.episode_index)
                    .map(|value| (value * 100.0).round() as i64 + 10_000)
                    .unwrap_or(10_000)
            } else {
                candidate
                    .episode_end_index
                    .or(candidate.episode_index)
                    .map(|value| (value * 100.0).round() as i64)
                    .unwrap_or(0)
            }
        }
    };
    let score_weight = (candidate.score * 100.0).round() as i64;
    let quality_weight = if candidate.is_collection { 1 } else { 0 };
    let freshness_weight = candidate
        .source_created_at
        .chars()
        .filter(|character| character.is_ascii_digit())
        .take(14)
        .collect::<String>()
        .parse::<i64>()
        .unwrap_or_default();

    (slot_weight, score_weight, quality_weight, freshness_weight)
}

fn evaluate_candidate(
    resource: &AnimeGardenResource,
    rules: &[FansubRuleDto],
    previous_selected: Option<&ResourceCandidateDto>,
    policy: &PolicyDto,
    release_status: &str,
) -> CandidateEvaluation {
    let resolution = extract_resolution(resource);
    let locale_hint = detect_locale_hint(resource);
    let is_raw = detect_raw(resource, locale_hint.as_deref());
    let normalized_fansub = resource.fansub_name.as_deref().map(normalize_name);

    if let Some(rule) = rules.iter().find(|rule| {
        normalized_fansub
            .as_deref()
            .is_some_and(|fansub| fansub == normalize_name(&rule.fansub_name))
            && rule.is_blacklist
    }) {
        return CandidateEvaluation {
            score: -1000.0,
            resolution,
            locale_hint,
            is_raw,
            rejected_reason: Some(format!("blocked by fansub rule: {}", rule.fansub_name)),
        };
    }

    let matched_rule = rules.iter().find(|rule| {
        normalized_fansub
            .as_deref()
            .is_some_and(|fansub| fansub == normalize_name(&rule.fansub_name))
            && !rule.is_blacklist
    });

    let mut score = 0.0;

    score += match release_status {
        "airing" => match resource.release_type.as_str() {
            "动画" => 18.0,
            "RAW" => -18.0,
            "合集" => -8.0,
            _ => 0.0,
        },
        "upcoming" => match resource.release_type.as_str() {
            "动画" => 12.0,
            "RAW" => -12.0,
            _ => 0.0,
        },
        _ => match resource.release_type.as_str() {
            "合集" => 24.0,
            "动画" => 12.0,
            "RAW" => -10.0,
            _ => 0.0,
        },
    };

    score += match resolution.as_deref() {
        Some("2160p") => 16.0,
        Some("1080p") => 12.0,
        Some("720p") => 8.0,
        Some("540p") => 4.0,
        _ => 0.0,
    };

    score += match locale_hint.as_deref() {
        Some("zh-Hans") => 16.0,
        Some("zh-Hant") => 14.0,
        Some("bilingual") => 12.0,
        Some("ja") => -4.0,
        _ => 0.0,
    };

    if is_raw {
        score -= 18.0;
    }

    if let Some(rule) = matched_rule {
        score += 400.0 + (rule.priority.max(0) as f64) * 200.0;
        score += locale_preference_bonus(&rule.locale_preference, locale_hint.as_deref(), is_raw);
    }

    if policy.prefer_same_fansub
        && previous_selected
            .and_then(|candidate| candidate.fansub_name.as_deref())
            .zip(resource.fansub_name.as_deref())
            .is_some_and(|(left, right)| normalize_name(left) == normalize_name(right))
    {
        score += 160.0;
    }

    CandidateEvaluation {
        score,
        resolution,
        locale_hint,
        is_raw,
        rejected_reason: None,
    }
}

fn locale_preference_bonus(preference: &str, locale_hint: Option<&str>, is_raw: bool) -> f64 {
    let preference = preference.to_lowercase();

    if preference.contains("raw") || preference.contains("ja") {
        return if is_raw { 20.0 } else { -4.0 };
    }

    match locale_hint {
        Some("zh-Hans") if preference.contains("hans") || preference.contains("简") => 22.0,
        Some("zh-Hant") if preference.contains("hant") || preference.contains("繁") => 22.0,
        Some("bilingual") if preference.contains("hans") || preference.contains("hant") => 12.0,
        Some("ja") => -12.0,
        _ => 0.0,
    }
}

fn detect_locale_hint(resource: &AnimeGardenResource) -> Option<String> {
    if let Some(language) = resource.parsed_language.as_deref() {
        let normalized = language.to_ascii_lowercase();
        if language.contains("简") && language.contains("繁") {
            return Some("bilingual".to_owned());
        }
        if normalized.contains("chs") || normalized.contains("gb") || language.contains("简") {
            return Some("zh-Hans".to_owned());
        }
        if normalized.contains("cht") || normalized.contains("big5") || language.contains("繁") {
            return Some("zh-Hant".to_owned());
        }
        if normalized.contains("jpn") || normalized == "ja" || language.contains("日") {
            return Some("ja".to_owned());
        }
    }

    detect_locale_hint_from_title(&resource.title)
}

fn detect_locale_hint_from_title(title: &str) -> Option<String> {
    let lower = title.to_lowercase();

    if ["简繁", "繁简", "双语", "chs&cht"]
        .iter()
        .any(|term| lower.contains(term))
    {
        return Some("bilingual".to_owned());
    }

    if ["简中", "简体", "gb", "chs"]
        .iter()
        .any(|term| lower.contains(term))
    {
        return Some("zh-Hans".to_owned());
    }

    if ["繁中", "繁體", "big5", "cht"]
        .iter()
        .any(|term| lower.contains(term))
    {
        return Some("zh-Hant".to_owned());
    }

    if ["日语", "日文", "jp"]
        .iter()
        .any(|term| lower.contains(term))
    {
        return Some("ja".to_owned());
    }

    None
}

fn detect_raw(resource: &AnimeGardenResource, locale_hint: Option<&str>) -> bool {
    let lower = resource.title.to_lowercase();

    lower.contains("raw")
        || lower.contains("生肉")
        || lower.contains("无字")
        || resource
            .parsed_subtitles
            .as_deref()
            .is_some_and(|subtitles| subtitles.eq_ignore_ascii_case("raw"))
        || locale_hint == Some("ja")
}

fn extract_resolution(resource: &AnimeGardenResource) -> Option<String> {
    if let Some(resolution) = resource.parsed_resolution.as_deref() {
        return Some(resolution.to_lowercase());
    }

    let upper = resource.title.to_uppercase();

    ["2160P", "1080P", "720P", "540P", "480P"]
        .iter()
        .find(|resolution| upper.contains(**resolution))
        .map(|resolution| resolution.to_lowercase())
}

pub(crate) fn infer_season_hint_from_texts<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> Option<i64> {
    values
        .into_iter()
        .filter_map(infer_season_hint_from_text)
        .max()
}

fn infer_season_hint_from_text(value: &str) -> Option<i64> {
    for regex in [
        season_suffix_regex(),
        japanese_season_regex(),
        english_season_regex(),
    ] {
        if let Some(captures) = regex.captures(value) {
            for index in 1..captures.len() {
                let Some(group) = captures.get(index) else {
                    continue;
                };
                if let Some(parsed) = parse_season_capture(group.as_str()) {
                    return Some(parsed);
                }
            }
        }
    }

    None
}

fn parse_season_capture(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if let Ok(parsed) = trimmed.parse::<i64>() {
        return (parsed > 0).then_some(parsed);
    }

    chinese_numeral_to_i64(trimmed)
}

fn chinese_numeral_to_i64(value: &str) -> Option<i64> {
    let normalized = value.trim();
    match normalized {
        "一" => Some(1),
        "二" | "两" | "兩" => Some(2),
        "三" => Some(3),
        "四" => Some(4),
        "五" => Some(5),
        "六" => Some(6),
        "七" => Some(7),
        "八" => Some(8),
        "九" => Some(9),
        "十" => Some(10),
        _ => {
            if let Some(prefix) = normalized.strip_suffix('十') {
                return chinese_numeral_to_i64(prefix).map(|value| value * 10);
            }
            if let Some((tens, ones)) = normalized.split_once('十') {
                let tens = if tens.is_empty() {
                    1
                } else {
                    chinese_numeral_to_i64(tens)?
                };
                let ones = if ones.is_empty() {
                    0
                } else {
                    chinese_numeral_to_i64(ones)?
                };
                Some(tens * 10 + ones)
            } else {
                None
            }
        }
    }
}

fn season_suffix_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\bS(?:eason)?\s*([0-9]{1,2})\b").expect("valid season suffix regex")
    })
}

fn japanese_season_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"第\s*([0-9]{1,2}|[一二三四五六七八九十两兩]+)\s*[季期]")
            .expect("valid japanese season regex")
    })
}

fn english_season_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b([0-9]{1,2})(?:st|nd|rd|th)?\s+season\b")
            .expect("valid english season regex")
    })
}

fn normalize_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_whitespace() && !matches!(character, '(' | ')' | '[' | ']')
        })
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn within_replacement_window(selection_updated_at: Option<&str>, hours: i64) -> bool {
    let Some(selection_updated_at) = selection_updated_at else {
        return true;
    };

    let Ok(parsed) = DateTime::parse_from_rfc3339(selection_updated_at) else {
        return true;
    };

    let deadline = parsed.with_timezone(&Utc) + Duration::hours(hours.max(0));
    Utc::now() <= deadline
}

#[cfg(test)]
mod tests {
    use super::{infer_season_hint_from_texts, normalize_resource_release_slots};
    use crate::animegarden::{AnimeGardenResource, AnimeGardenSearchProfile};

    #[test]
    fn parses_subject_season_hints_from_common_titles() {
        assert_eq!(
            infer_season_hint_from_texts(["【我推的孩子】 第三季", "Oshi no Ko 3rd Season"]),
            Some(3)
        );
        assert_eq!(
            infer_season_hint_from_texts(["Example Title S2", "Example Title Season 2"]),
            Some(2)
        );
    }

    #[test]
    fn normalizes_absolute_episode_numbers_back_to_current_season() {
        let profile = AnimeGardenSearchProfile {
            bangumi_subject_id: 517057,
            title: "【推しの子】 第3期".to_owned(),
            title_cn: "【我推的孩子】 第三季".to_owned(),
            season_hint: Some(3),
        };
        let resources = vec![
            sample_resource(
                "[LoliHouse] 【我推的孩子】 第三季 / Oshi no Ko S3 - 07 [1080p]",
                "2026-02-25T15:09:08.949Z",
                Some(7.0),
                Some(3),
            ),
            sample_resource(
                "[ANi] 【OSHI NO KO】 - 【我推的孩子】 - 31 [1080P]",
                "2026-02-25T15:02:13.557Z",
                Some(31.0),
                None,
            ),
            sample_resource(
                "[LoliHouse] 【我推的孩子】 第三季 / Oshi no Ko S3 - 08 [1080p]",
                "2026-03-04T15:12:17.937Z",
                Some(8.0),
                Some(3),
            ),
            sample_resource(
                "[ANi] 【OSHI NO KO】 - 【我推的孩子】 - 32 [1080P]",
                "2026-03-04T15:02:05.000Z",
                Some(32.0),
                None,
            ),
        ];

        let normalized = normalize_resource_release_slots(resources, &profile, "airing");
        let slot_keys = normalized
            .into_iter()
            .map(|item| item.release_slot.slot_key)
            .collect::<Vec<_>>();

        assert_eq!(
            slot_keys,
            vec![
                "episode:7".to_owned(),
                "episode:7".to_owned(),
                "episode:8".to_owned(),
                "episode:8".to_owned(),
            ]
        );
    }

    fn sample_resource(
        title: &str,
        created_at: &str,
        parsed_episode_number: Option<f64>,
        parsed_season_number: Option<i64>,
    ) -> AnimeGardenResource {
        AnimeGardenResource {
            provider: "dmhy".to_owned(),
            provider_id: title.to_owned(),
            title: title.to_owned(),
            href: String::new(),
            release_type: "动画".to_owned(),
            magnet: String::new(),
            size: 0,
            created_at: created_at.to_owned(),
            fetched_at: created_at.to_owned(),
            fansub_name: None,
            publisher_name: String::new(),
            parsed_episode_number,
            parsed_episode_end_number: parsed_episode_number,
            parsed_season_number,
            parsed_resolution: Some("1080P".to_owned()),
            parsed_language: None,
            parsed_subtitles: None,
        }
    }
}
