use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;

use crate::{
    animegarden::{AnimeGardenClient, AnimeGardenResource, AnimeGardenSearchProfile},
    db::{self, NewResourceCandidate},
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
    ) -> Result<Vec<ResourceCandidateDto>, AppError> {
        let search = self.animegarden.search_resources(profile).await?;
        let search_run_id =
            db::start_resource_search_run(pool, job.id, job.bangumi_subject_id, &search.strategy)
                .await?;

        let rules = db::list_fansub_rules(pool).await?;
        let previous_selected =
            db::latest_selected_candidate_for_subject(pool, job.bangumi_subject_id).await?;
        let current_selected = db::current_selected_candidate_for_job(pool, job.id).await?;

        let mut stored = Vec::new();
        for resource in search.resources {
            let evaluation = evaluate_candidate(
                &resource,
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
                    provider: resource.provider,
                    provider_resource_id: resource.provider_id,
                    title: resource.title,
                    href: resource.href,
                    magnet: resource.magnet,
                    release_type: resource.release_type,
                    size_bytes: resource.size,
                    fansub_name: resource.fansub_name,
                    publisher_name: resource.publisher_name,
                    source_created_at: resource.created_at,
                    source_fetched_at: resource.fetched_at,
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
            left.score
                .partial_cmp(&right.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.source_created_at.cmp(&right.source_created_at))
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

        if best.score <= current.score {
            return (
                Some(current.id),
                "retained",
                "Current selected candidate remains preferred within replacement window".to_owned(),
            );
        }
    }

    (
        Some(best.id),
        "selected",
        "Selected best available resource candidate".to_owned(),
    )
}

fn evaluate_candidate(
    resource: &AnimeGardenResource,
    rules: &[FansubRuleDto],
    previous_selected: Option<&ResourceCandidateDto>,
    policy: &PolicyDto,
    release_status: &str,
) -> CandidateEvaluation {
    let resolution = extract_resolution(&resource.title);
    let locale_hint = detect_locale_hint(&resource.title);
    let is_raw = detect_raw(&resource.title, locale_hint.as_deref());
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
        score += (rule.priority.max(0) as f64) * 2.0;
        score += locale_preference_bonus(&rule.locale_preference, locale_hint.as_deref(), is_raw);
    }

    if policy.prefer_same_fansub
        && previous_selected
            .and_then(|candidate| candidate.fansub_name.as_deref())
            .zip(resource.fansub_name.as_deref())
            .is_some_and(|(left, right)| normalize_name(left) == normalize_name(right))
    {
        score += 28.0;
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

fn detect_locale_hint(title: &str) -> Option<String> {
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

fn detect_raw(title: &str, locale_hint: Option<&str>) -> bool {
    let lower = title.to_lowercase();

    lower.contains("raw")
        || lower.contains("生肉")
        || lower.contains("无字")
        || locale_hint == Some("ja")
}

fn extract_resolution(title: &str) -> Option<String> {
    let upper = title.to_uppercase();

    ["2160P", "1080P", "720P", "540P", "480P"]
        .iter()
        .find(|resolution| upper.contains(**resolution))
        .map(|resolution| resolution.to_lowercase())
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

fn within_replacement_window(selection_updated_at: Option<&str>, hours: i64) -> bool {
    let Some(selection_updated_at) = selection_updated_at else {
        return true;
    };

    let Ok(parsed) = DateTime::parse_from_rfc3339(selection_updated_at) else {
        return true;
    };

    let deadline = parsed.with_timezone(&Utc) + Duration::hours(hours.max(0));
    Utc::now() <= deadline
}
