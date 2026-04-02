use std::collections::HashSet;

use anicargo_metadata_parser::parse_release_name;

use crate::{
    bangumi::{BangumiClient, SubjectRaw},
    discovery::{infer_part_hint_from_texts, infer_season_hint_from_texts},
    types::AppError,
};

#[derive(Debug, Clone)]
pub struct SubjectPartSegment {
    pub bangumi_subject_id: i64,
    pub total_episodes: i64,
    pub part_index: i64,
    pub global_start: f64,
    pub global_end: f64,
}

#[derive(Debug, Clone)]
pub struct SubjectPartGroup {
    pub segments: Vec<SubjectPartSegment>,
}

#[derive(Debug, Clone)]
struct SubjectIdentity {
    bangumi_subject_id: i64,
    base_titles: Vec<String>,
    season_hint: Option<i64>,
    part_hint: Option<i64>,
    total_episodes: i64,
}

pub fn collect_base_title_aliases(title: &str, title_cn: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut seen = HashSet::new();

    for value in [title, title_cn] {
        for alias in extract_base_titles(value) {
            let normalized = alias.trim();
            if normalized.is_empty() {
                continue;
            }
            let dedupe_key = normalized.to_lowercase();
            if seen.insert(dedupe_key) {
                values.push(normalized.to_owned());
            }
        }
    }

    values
}

pub async fn resolve_subject_part_group(
    bangumi: &BangumiClient,
    subject_id: i64,
) -> Result<Option<SubjectPartGroup>, AppError> {
    let current = bangumi.fetch_subject(subject_id).await?;
    let current_identity = SubjectIdentity::from_subject(&current);
    let related = bangumi.fetch_related_subjects(subject_id).await?;

    let mut identities = vec![current_identity.clone()];
    for item in related {
        if item.r#type != 2 {
            continue;
        }
        if !matches!(
            item.relation.trim(),
            "\u{524D}\u{4F20}" | "\u{7EED}\u{96C6}"
        ) {
            continue;
        }

        let subject = match bangumi.fetch_subject(item.id).await {
            Ok(subject) => subject,
            Err(error) => {
                tracing::warn!(
                    subject_id,
                    related_subject_id = item.id,
                    relation = %item.relation,
                    error = %error,
                    "Failed to fetch Bangumi related subject while resolving split-part group"
                );
                continue;
            }
        };
        let identity = SubjectIdentity::from_subject(&subject);
        if looks_like_split_part_peer(&current_identity, &identity) {
            identities.push(identity);
        }
    }

    if identities.len() <= 1
        || !identities
            .iter()
            .any(|identity| identity.part_hint.unwrap_or(1) > 1)
    {
        return Ok(None);
    }

    let mut segments = identities
        .into_iter()
        .map(|identity| SubjectPartSegment {
            bangumi_subject_id: identity.bangumi_subject_id,
            total_episodes: identity.total_episodes.max(0),
            part_index: identity.part_hint.unwrap_or(1),
            global_start: 0.0,
            global_end: 0.0,
        })
        .collect::<Vec<_>>();

    segments.sort_by(|left, right| {
        left.part_index
            .cmp(&right.part_index)
            .then(left.bangumi_subject_id.cmp(&right.bangumi_subject_id))
    });
    segments.dedup_by(|left, right| left.bangumi_subject_id == right.bangumi_subject_id);

    if !segments
        .iter()
        .any(|segment| segment.bangumi_subject_id == subject_id)
    {
        return Ok(None);
    }

    let mut cursor = 1.0;
    for segment in &mut segments {
        let width = segment.total_episodes.max(0) as f64;
        if width <= 0.0 {
            return Ok(None);
        }
        segment.global_start = cursor;
        segment.global_end = cursor + width - 1.0;
        cursor = segment.global_end + 1.0;
    }

    Ok(Some(SubjectPartGroup { segments }))
}

pub fn current_segment<'a>(
    group: &'a SubjectPartGroup,
    bangumi_subject_id: i64,
) -> Option<&'a SubjectPartSegment> {
    group.segments
        .iter()
        .find(|segment| segment.bangumi_subject_id == bangumi_subject_id)
}

pub fn first_segment(group: &SubjectPartGroup) -> Option<&SubjectPartSegment> {
    group.segments.first()
}

pub fn last_segment(group: &SubjectPartGroup) -> Option<&SubjectPartSegment> {
    group.segments.last()
}

pub fn map_global_episode_to_segment(
    group: &SubjectPartGroup,
    global_episode: f64,
) -> Option<(i64, f64)> {
    let segment = group.segments.iter().find(|segment| {
        global_episode + 0.001 >= segment.global_start
            && global_episode - 0.001 <= segment.global_end
    })?;
    Some((
        segment.bangumi_subject_id,
        global_episode - segment.global_start + 1.0,
    ))
}

pub fn map_global_range_to_segments(
    group: &SubjectPartGroup,
    global_start: f64,
    global_end: f64,
) -> Vec<(i64, f64, f64)> {
    if global_end < global_start {
        return Vec::new();
    }

    group
        .segments
        .iter()
        .filter_map(|segment| {
            let overlap_start = global_start.max(segment.global_start);
            let overlap_end = global_end.min(segment.global_end);
            if overlap_end + 0.001 < overlap_start {
                return None;
            }
            Some((
                segment.bangumi_subject_id,
                overlap_start - segment.global_start + 1.0,
                overlap_end - segment.global_start + 1.0,
            ))
        })
        .collect()
}

fn looks_like_split_part_peer(current: &SubjectIdentity, candidate: &SubjectIdentity) -> bool {
    if current.bangumi_subject_id == candidate.bangumi_subject_id {
        return false;
    }

    if current.part_hint.is_none() && candidate.part_hint.is_none() {
        return false;
    }

    if let (Some(current_season), Some(candidate_season)) =
        (current.season_hint, candidate.season_hint)
    {
        if current_season != candidate_season {
            return false;
        }
    }

    current
        .base_titles
        .iter()
        .any(|left| candidate.base_titles.iter().any(|right| left == right))
}

impl SubjectIdentity {
    fn from_subject(subject: &SubjectRaw) -> Self {
        Self {
            bangumi_subject_id: subject.id,
            base_titles: collect_base_title_aliases(&subject.name, &subject.name_cn),
            season_hint: infer_season_hint_from_texts([subject.name.as_str(), subject.name_cn.as_str()]),
            part_hint: infer_part_hint_from_texts([subject.name.as_str(), subject.name_cn.as_str()]),
            total_episodes: subject.total_episodes.unwrap_or_default(),
        }
    }
}

fn extract_base_titles(value: &str) -> Vec<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Vec::new();
    }

    let parsed = parse_release_name(normalized);
    let mut values = Vec::new();
    for candidate in [
        parsed.titles.primary.as_deref(),
        parsed.titles.cjk.as_deref(),
        parsed.titles.latin.as_deref(),
        parsed.titles.japanese.as_deref(),
    ] {
        let Some(candidate) = candidate else {
            continue;
        };
        let cleaned = candidate.trim();
        if cleaned.is_empty() {
            continue;
        }
        let key = normalize_base_title(cleaned);
        if !key.is_empty() {
            values.push(key);
        }
    }

    if values.is_empty() {
        let key = normalize_base_title(normalized);
        if !key.is_empty() {
            values.push(key);
        }
    }

    values.sort();
    values.dedup();
    values
}

fn normalize_base_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric() || is_cjk_like(*character))
        .flat_map(|character| character.to_lowercase())
        .collect::<String>()
}

fn is_cjk_like(character: char) -> bool {
    matches!(
        character as u32,
        0x3040..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
    )
}

#[cfg(test)]
mod tests {
    use super::{
        SubjectPartGroup, SubjectPartSegment, collect_base_title_aliases, first_segment,
        last_segment, map_global_episode_to_segment, map_global_range_to_segments,
    };

    #[test]
    fn strips_part_markers_from_subject_aliases() {
        let aliases = collect_base_title_aliases(
            "关于我转生变成史莱姆这档事 第二季 第2部分",
            "転生したらスライムだった件 第2期 2",
        );

        assert!(
            aliases
                .iter()
                .any(|item| item.contains("关于我转生变成史莱姆这档事"))
        );
        assert!(
            aliases
                .iter()
                .any(|item| item.contains("転生したらスライムだった件"))
        );
    }

    #[test]
    fn maps_global_episode_numbers_into_part_segments() {
        let group = SubjectPartGroup {
            segments: vec![
                SubjectPartSegment {
                    bangumi_subject_id: 1,
                    total_episodes: 11,
                    part_index: 1,
                    global_start: 1.0,
                    global_end: 11.0,
                },
                SubjectPartSegment {
                    bangumi_subject_id: 2,
                    total_episodes: 12,
                    part_index: 2,
                    global_start: 12.0,
                    global_end: 23.0,
                },
            ],
        };

        assert_eq!(map_global_episode_to_segment(&group, 3.0), Some((1, 3.0)));
        assert_eq!(map_global_episode_to_segment(&group, 12.0), Some((2, 1.0)));
        assert_eq!(map_global_episode_to_segment(&group, 23.0), Some((2, 12.0)));
    }

    #[test]
    fn splits_global_batch_ranges_into_part_windows() {
        let group = SubjectPartGroup {
            segments: vec![
                SubjectPartSegment {
                    bangumi_subject_id: 1,
                    total_episodes: 11,
                    part_index: 1,
                    global_start: 1.0,
                    global_end: 11.0,
                },
                SubjectPartSegment {
                    bangumi_subject_id: 2,
                    total_episodes: 12,
                    part_index: 2,
                    global_start: 12.0,
                    global_end: 23.0,
                },
            ],
        };

        assert_eq!(
            map_global_range_to_segments(&group, 1.0, 23.0),
            vec![(1, 1.0, 11.0), (2, 1.0, 12.0)]
        );
    }

    #[test]
    fn exposes_group_bounds() {
        let group = SubjectPartGroup {
            segments: vec![
                SubjectPartSegment {
                    bangumi_subject_id: 1,
                    total_episodes: 11,
                    part_index: 1,
                    global_start: 1.0,
                    global_end: 11.0,
                },
                SubjectPartSegment {
                    bangumi_subject_id: 2,
                    total_episodes: 12,
                    part_index: 2,
                    global_start: 12.0,
                    global_end: 23.0,
                },
            ],
        };

        assert_eq!(first_segment(&group).map(|item| item.global_start), Some(1.0));
        assert_eq!(last_segment(&group).map(|item| item.global_end), Some(23.0));
    }
}
