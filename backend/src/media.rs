use std::{
    fs,
    process::Command,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anicargo_metadata_parser::{
    EpisodeDescriptor, EpisodeNumber, EpisodeRangeDescriptor, ParseResult, parse_file_name,
    parse_release_name,
};
use anyhow::Context;
use regex::Regex;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct ParsedReleaseSlot {
    pub slot_key: String,
    pub episode_index: Option<f64>,
    pub episode_end_index: Option<f64>,
    pub is_collection: bool,
}

#[derive(Debug, Clone)]
pub struct IndexedMediaFile {
    pub slot_key: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub file_name: String,
    pub file_ext: String,
    pub size_bytes: i64,
    pub episode_index: Option<f64>,
    pub episode_end_index: Option<f64>,
    pub is_collection: bool,
}

#[derive(Debug, Clone)]
pub struct PreparedSubtitleTrack {
    pub id: String,
    pub label: String,
    pub language: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct PreparedSubtitleAsset {
    pub path: PathBuf,
}

pub fn infer_release_slot(
    title: &str,
    release_type: &str,
    provider_resource_id: &str,
    release_status: &str,
) -> ParsedReleaseSlot {
    let parsed = parse_release_name(title);
    if let Some(slot) = slot_from_parse(&parsed) {
        return slot;
    }

    infer_release_slot_fallback(title, release_type, provider_resource_id, release_status)
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
                slot_key: inferred_slot.slot_key.clone(),
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

pub fn probe_subtitle_tracks(media_path: &Path) -> anyhow::Result<Vec<PreparedSubtitleTrack>> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_streams")
        .arg(media_path)
        .output()
        .with_context(|| {
            format!(
                "failed to launch ffprobe while probing subtitle tracks for {}",
                media_path.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        anyhow::bail!(
            "ffprobe failed while probing subtitle tracks for {}: {}",
            media_path.display(),
            if stderr.is_empty() { "unknown error" } else { &stderr }
        );
    }

    let parsed = serde_json::from_slice::<FfprobeOutput>(&output.stdout).with_context(|| {
        format!(
            "failed to parse ffprobe subtitle probe output for {}",
            media_path.display()
        )
    })?;

    Ok(parsed
        .streams
        .into_iter()
        .filter_map(|stream| stream.to_prepared_track())
        .collect())
}

pub fn materialize_subtitle_track(
    media_path: &Path,
    media_root: &Path,
    media_inventory_id: i64,
    track_id: &str,
) -> anyhow::Result<PreparedSubtitleAsset> {
    let stream_index = parse_embedded_track_id(track_id)?;
    let subtitle_root = media_root
        .join("_subtitles")
        .join(media_inventory_id.to_string());
    fs::create_dir_all(&subtitle_root).with_context(|| {
        format!(
            "failed to create subtitle cache directory {}",
            subtitle_root.display()
        )
    })?;

    let output_path = subtitle_root.join(format!("stream-{stream_index}.vtt"));
    if output_path.exists() {
        return Ok(PreparedSubtitleAsset { path: output_path });
    }

    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(media_path)
        .arg("-map")
        .arg(format!("0:{stream_index}"))
        .arg("-c:s")
        .arg("webvtt")
        .arg(&output_path)
        .output()
        .with_context(|| {
            format!(
                "failed to launch ffmpeg while extracting subtitle track {} for {}",
                track_id,
                media_path.display()
            )
        })?;

    if !output.status.success() {
        let _ = fs::remove_file(&output_path);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        anyhow::bail!(
            "ffmpeg failed while extracting subtitle track {} for {}: {}",
            track_id,
            media_path.display(),
            if stderr.is_empty() { "unknown error" } else { &stderr }
        );
    }

    Ok(PreparedSubtitleAsset { path: output_path })
}

pub fn parse_embedded_track_id(track_id: &str) -> anyhow::Result<i32> {
    let raw = track_id
        .strip_prefix("stream-")
        .ok_or_else(|| anyhow::anyhow!("unsupported subtitle track id '{track_id}'"))?;
    raw.parse::<i32>()
        .with_context(|| format!("invalid subtitle stream index in track id '{track_id}'"))
}

fn infer_file_slot(file_name: &str, fallback_slot: &ParsedReleaseSlot) -> ParsedReleaseSlot {
    let parsed = parse_file_name(file_name);
    if let Some(slot) = slot_from_parse(&parsed) {
        return slot;
    }

    let stem = PathBuf::from(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name)
        .to_owned();

    if let Some(episode) = extract_single_episode(&stem) {
        return ParsedReleaseSlot {
            slot_key: format!("episode:{}", format_episode_number(episode)),
            episode_index: Some(episode),
            episode_end_index: Some(episode),
            is_collection: false,
        };
    }

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

    fallback_slot.clone()
}

fn slot_from_parse(parsed: &ParseResult) -> Option<ParsedReleaseSlot> {
    if let Some(range) = parsed.episode_range.as_ref() {
        return slot_from_range(range);
    }

    parsed.episode.as_ref().map(slot_from_episode)
}

fn slot_from_episode(descriptor: &EpisodeDescriptor) -> ParsedReleaseSlot {
    let primary = descriptor.primary.decimal();
    let secondary = descriptor.secondary.map(EpisodeNumber::decimal);
    let selected = match secondary {
        Some(value) if primary <= 0.0 && value > 0.0 => value,
        Some(value) if value > 0.0 && primary > 0.0 => primary.min(value),
        Some(value) => value,
        None => primary,
    };

    ParsedReleaseSlot {
        slot_key: format!("episode:{}", format_episode_number(selected)),
        episode_index: Some(selected),
        episode_end_index: Some(selected),
        is_collection: false,
    }
}

fn slot_from_range(descriptor: &EpisodeRangeDescriptor) -> Option<ParsedReleaseSlot> {
    let primary_start = descriptor.primary_start.decimal();
    let primary_end = descriptor.primary_end.decimal();

    let mut best = (primary_start, primary_end);
    if let (Some(secondary_start), Some(secondary_end)) =
        (descriptor.secondary_start, descriptor.secondary_end)
    {
        let secondary = (secondary_start.decimal(), secondary_end.decimal());
        if secondary.0 < best.0
            || ((secondary.0 - best.0).abs() < f64::EPSILON && secondary.1 < best.1)
        {
            best = secondary;
        }
    }

    if best.1 < best.0 {
        return None;
    }

    Some(ParsedReleaseSlot {
        slot_key: format!(
            "batch:{}-{}",
            format_episode_number(best.0),
            format_episode_number(best.1)
        ),
        episode_index: Some(best.0),
        episode_end_index: Some(best.1),
        is_collection: true,
    })
}

fn infer_release_slot_fallback(
    title: &str,
    release_type: &str,
    provider_resource_id: &str,
    release_status: &str,
) -> ParsedReleaseSlot {
    if let Some(episode) = extract_single_episode(title) {
        return ParsedReleaseSlot {
            slot_key: format!("episode:{}", format_episode_number(episode)),
            episode_index: Some(episode),
            episode_end_index: Some(episode),
            is_collection: false,
        };
    }

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

    let lowered_release_type = release_type.to_ascii_lowercase();
    if lowered_release_type.contains("batch")
        || lowered_release_type.contains("collection")
        || lowered_release_type.contains("complete")
    {
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
        dashed_episode_regex(),
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
    let trimmed = value.trim();
    let trimmed = trimmed.trim_start_matches('0');
    let normalized = if trimmed.is_empty() { "0" } else { trimmed };
    let parsed = normalized.parse::<f64>().ok()?;
    (parsed >= 0.0 && parsed <= 500.0).then_some(parsed)
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

fn is_text_subtitle_codec(codec_name: &str) -> bool {
    matches!(
        codec_name,
        "subrip" | "ass" | "ssa" | "webvtt" | "mov_text" | "text"
    )
}

fn build_subtitle_label(stream: &FfprobeStream) -> String {
    if let Some(title) = stream
        .tags
        .as_ref()
        .and_then(|tags| tags.title.as_deref())
        .map(str::trim)
        .filter(|title| !title.is_empty())
    {
        return title.to_owned();
    }

    if let Some(language) = stream
        .tags
        .as_ref()
        .and_then(|tags| tags.language.as_deref())
        .map(str::trim)
        .filter(|language| !language.is_empty())
    {
        return format!("Subtitle {language}");
    }

    format!("Subtitle {}", stream.index)
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    index: i32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    #[serde(default)]
    tags: Option<FfprobeTags>,
}

#[derive(Debug, Deserialize, Default)]
struct FfprobeTags {
    language: Option<String>,
    title: Option<String>,
}

impl FfprobeStream {
    fn to_prepared_track(self) -> Option<PreparedSubtitleTrack> {
        if self.codec_type.as_deref() != Some("subtitle") {
            return None;
        }

        let codec_name = self.codec_name.as_deref()?;
        if !is_text_subtitle_codec(codec_name) {
            return None;
        }

        Some(PreparedSubtitleTrack {
            id: format!("stream-{}", self.index),
            label: build_subtitle_label(&self),
            language: self.tags.as_ref().and_then(|tags| tags.language.clone()),
            kind: "embedded".to_owned(),
        })
    }
}

fn collection_range_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)(?:^|[^0-9A-Za-z])(\d{1,3}(?:\.\d+)?)\s*[-~]\s*(\d{1,3}(?:\.\d+)?)(?:\s*(?:END|FIN))?(?:[^0-9A-Za-z]|$)",
        )
        .expect("valid collection range regex")
    })
}

fn collection_total_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)(?:all|complete|batch)\s*(\d{1,3}(?:\.\d+)?)")
            .expect("valid collection total regex")
    })
}

fn explicit_episode_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?ix)
            \bEP?\s*[\.\-]?\s*(\d{1,3}(?:\.\d+)?)\b
            |
            \bEpisode\s*(\d{1,3}(?:\.\d+)?)\b
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

fn dashed_episode_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?ix)(?:^|[^0-9A-Za-z])-\s*(\d{1,3}(?:\.\d+)?)(?:\s*(?:END|FIN))?(?:\s*(?:[\[\(].*)?)$",
        )
        .expect("valid dashed episode regex")
    })
}

fn suffix_episode_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(\d{1,3}(?:\.\d+)?)\s*(?:END|FIN)\b")
            .expect("valid suffix episode regex")
    })
}

#[cfg(test)]
mod tests {
    use super::{extract_collection_span, infer_release_slot, scan_video_files, slot_from_parse};
    use crate::media::ParsedReleaseSlot;
    use anicargo_metadata_parser::{parse_file_name, parse_release_name};
    use std::{fs, io::Write};

    #[test]
    fn parser_prefers_local_episode_alias_over_absolute_number() {
        let slot = infer_release_slot(
            "[LoliHouse] Tensei Shitara Slime Datta Ken 3rd Season - 24(72) [WebRip 1080p]",
            "single",
            "example",
            "airing",
        );
        assert_eq!(slot.slot_key, "episode:24");
        assert_eq!(slot.episode_index, Some(24.0));
        assert!(!slot.is_collection);
    }

    #[test]
    fn parser_prefers_local_batch_range_when_dual_range_exists() {
        let parsed = parse_release_name(
            "[LoliHouse] Tensei Shitara Slime Datta Ken 3rd Season [48.5-72(00-24) Batch][WebRip 1080p][Fin]",
        );
        let slot = slot_from_parse(&parsed).expect("slot");
        assert_eq!(slot.slot_key, "batch:0-24");
        assert_eq!(slot.episode_index, Some(0.0));
        assert_eq!(slot.episode_end_index, Some(24.0));
        assert!(slot.is_collection);
    }

    #[test]
    fn still_parses_real_collection_ranges_in_fallback() {
        assert_eq!(
            extract_collection_span("[SubsPlease] Example Title [01-12] [1080p]"),
            Some((1.0, 12.0))
        );
    }

    #[test]
    fn file_indexing_uses_metadata_parser_for_dual_episode_names() {
        let root = std::env::temp_dir().join(format!("anicargo-media-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join("Tensei Shitara Slime Datta Ken 3rd Season - 24(72).mkv");
        let mut file = fs::File::create(&path).expect("create video");
        file.write_all(b"test").expect("write video");

        let fallback = ParsedReleaseSlot {
            slot_key: "batch:test".to_owned(),
            episode_index: None,
            episode_end_index: None,
            is_collection: true,
        };
        let indexed = scan_video_files(&root, &fallback).expect("scan media");
        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0].episode_index, Some(24.0));
        assert_eq!(indexed[0].episode_end_index, Some(24.0));
        assert!(!indexed[0].is_collection);

        fs::remove_dir_all(&root).expect("cleanup temp root");
    }

    #[test]
    fn file_parser_can_read_fractional_recap_alias() {
        let parsed = parse_file_name(
            "Tensei Shitara Slime Datta Ken 3rd Season - 00(48.5) [WebRip 1080p].mkv",
        );
        let slot = slot_from_parse(&parsed).expect("slot");
        assert_eq!(slot.episode_index, Some(48.5));
        assert_eq!(slot.slot_key, "episode:48.5");
    }
}
