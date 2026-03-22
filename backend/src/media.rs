use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::Context;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct ParsedReleaseSlot {
    pub slot_key: String,
    pub episode_index: Option<f64>,
    pub episode_end_index: Option<f64>,
    pub is_collection: bool,
}

#[derive(Debug, Clone)]
pub struct IndexedMediaFile {
    pub relative_path: String,
    pub absolute_path: String,
    pub file_name: String,
    pub file_ext: String,
    pub size_bytes: i64,
    pub episode_index: Option<f64>,
    pub episode_end_index: Option<f64>,
    pub is_collection: bool,
}

pub fn infer_release_slot(
    title: &str,
    release_type: &str,
    provider_resource_id: &str,
    release_status: &str,
) -> ParsedReleaseSlot {
    if let Some((start, end)) = extract_collection_span(title) {
        return ParsedReleaseSlot {
            slot_key: format!(
                "batch:{}-{}",
                format_episode_number(start),
                format_episode_number(end)
            ),
            episode_index: Some(start),
            episode_end_index: Some(end),
            is_collection: true,
        };
    }

    if let Some(episode) = extract_single_episode(title) {
        return ParsedReleaseSlot {
            slot_key: format!("episode:{}", format_episode_number(episode)),
            episode_index: Some(episode),
            episode_end_index: Some(episode),
            is_collection: false,
        };
    }

    let lowered_release_type = release_type.to_ascii_lowercase();
    if lowered_release_type.contains("batch") || lowered_release_type.contains("collection") {
        return ParsedReleaseSlot {
            slot_key: format!("batch:{}", sanitize_slot_fragment(provider_resource_id)),
            episode_index: None,
            episode_end_index: None,
            is_collection: true,
        };
    }

    let prefix = match release_status {
        "completed" => "pack",
        "airing" => "item",
        "upcoming" => "upcoming",
        _ => "item",
    };

    ParsedReleaseSlot {
        slot_key: format!("{prefix}:{}", sanitize_slot_fragment(provider_resource_id)),
        episode_index: None,
        episode_end_index: None,
        is_collection: false,
    }
}

pub fn scan_video_files(
    root: &Path,
    fallback_slot: &ParsedReleaseSlot,
) -> anyhow::Result<Vec<IndexedMediaFile>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current)
            .with_context(|| format!("failed to read media directory {}", current.display()))?;

        for entry in entries {
            let entry = entry.with_context(|| {
                format!("failed to read directory entry under {}", current.display())
            })?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to read file type for {}", path.display()))?;

            if file_type.is_dir() {
                stack.push(path);
                continue;
            }

            if !file_type.is_file() {
                continue;
            }

            let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };
            let file_ext = extension.to_ascii_lowercase();
            if !is_video_extension(&file_ext) {
                continue;
            }

            let metadata = entry.metadata().with_context(|| {
                format!("failed to read metadata for media file {}", path.display())
            })?;
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_owned();
            let relative_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let inferred_slot = infer_file_slot(&file_name, fallback_slot);

            files.push(IndexedMediaFile {
                relative_path,
                absolute_path: path.to_string_lossy().into_owned(),
                file_name,
                file_ext,
                size_bytes: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
                episode_index: inferred_slot.episode_index,
                episode_end_index: inferred_slot.episode_end_index,
                is_collection: inferred_slot.is_collection,
            });
        }
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn infer_file_slot(file_name: &str, fallback_slot: &ParsedReleaseSlot) -> ParsedReleaseSlot {
    let stem = PathBuf::from(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name)
        .to_owned();

    if let Some((start, end)) = extract_collection_span(&stem) {
        return ParsedReleaseSlot {
            slot_key: format!(
                "batch:{}-{}",
                format_episode_number(start),
                format_episode_number(end)
            ),
            episode_index: Some(start),
            episode_end_index: Some(end),
            is_collection: true,
        };
    }

    if let Some(episode) = extract_single_episode(&stem) {
        return ParsedReleaseSlot {
            slot_key: format!("episode:{}", format_episode_number(episode)),
            episode_index: Some(episode),
            episode_end_index: Some(episode),
            is_collection: false,
        };
    }

    fallback_slot.clone()
}

fn extract_collection_span(title: &str) -> Option<(f64, f64)> {
    if let Some(captures) = collection_range_regex().captures(title) {
        let start = parse_episode_capture(captures.get(1)?.as_str())?;
        let end = parse_episode_capture(captures.get(2)?.as_str())?;
        if end >= start {
            return Some((start, end));
        }
    }

    if let Some(captures) = collection_total_regex().captures(title) {
        let end = parse_episode_capture(captures.get(1)?.as_str())?;
        if end >= 1.0 {
            return Some((1.0, end));
        }
    }

    None
}

fn extract_single_episode(title: &str) -> Option<f64> {
    for regex in [
        explicit_episode_regex(),
        bracket_episode_regex(),
        suffix_episode_regex(),
    ] {
        if let Some(captures) = regex.captures(title) {
            for index in 1..captures.len() {
                let Some(value) = captures.get(index) else {
                    continue;
                };
                if let Some(parsed) = parse_episode_capture(value.as_str()) {
                    return Some(parsed);
                }
            }
        }
    }

    None
}

fn parse_episode_capture(value: &str) -> Option<f64> {
    let trimmed = value.trim().trim_matches('0');
    let normalized = if trimmed.is_empty() { "0" } else { trimmed };
    let parsed = normalized.parse::<f64>().ok()?;
    (parsed > 0.0 && parsed <= 500.0).then_some(parsed)
}

fn format_episode_number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

fn sanitize_slot_fragment(value: &str) -> String {
    let mut normalized = value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        .collect::<String>();

    if normalized.is_empty() {
        normalized = "unknown".to_owned();
    }

    normalized
}

fn is_video_extension(extension: &str) -> bool {
    matches!(
        extension,
        "mkv" | "mp4" | "avi" | "m2ts" | "ts" | "webm" | "mov" | "flv"
    )
}

fn collection_range_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)(?:^|[^0-9])(\d{1,3}(?:\.\d+)?)\s*[-~]\s*(\d{1,3}(?:\.\d+)?)(?:\s*(?:END|FIN))?(?:[^0-9]|$)")
            .expect("valid collection range regex")
    })
}

fn collection_total_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"全\s*(\d{1,3}(?:\.\d+)?)\s*[话話集]").expect("valid collection total regex")
    })
}

fn explicit_episode_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?ix)
            第 \s* (\d{1,3}(?:\.\d+)?) \s* [话話集]
            |
            \b E P? \s* [\.\-]? \s* (\d{1,3}(?:\.\d+)?) \b
        ",
        )
        .expect("valid explicit episode regex")
    })
}

fn bracket_episode_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\[(\d{1,3}(?:\.\d+)?)\]").expect("valid bracket episode regex")
    })
}

fn suffix_episode_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(\d{1,3}(?:\.\d+)?)\s*(?:END|FIN)\b")
            .expect("valid suffix episode regex")
    })
}
