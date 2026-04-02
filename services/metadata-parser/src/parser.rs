use std::sync::OnceLock;

use regex::Regex;

use crate::types::{
    AudioInfo, EpisodeDescriptor, EpisodeNumber, EpisodeRangeDescriptor, FansubInfo, FileInfo,
    FileRole, ParseFlags, ParseResult, ParseSourceKind, ScriptKind, SeasonInfo, SubtitleInfo,
    SubtitleStorage, TechnicalInfo, TitleCandidate, TitleInfo,
};

pub fn parse_release_name(input: &str) -> ParseResult {
    parse_impl(input, ParseSourceKind::ReleaseTitle)
}

pub fn parse_file_name(input: &str) -> ParseResult {
    parse_impl(input, ParseSourceKind::FileName)
}

fn parse_impl(input: &str, source_kind: ParseSourceKind) -> ParseResult {
    let raw = input.trim().to_owned();
    let file_name = match source_kind {
        ParseSourceKind::FileName => basename(&raw),
        ParseSourceKind::ReleaseTitle => raw.clone(),
    };
    let (stem, extension) = split_extension(&file_name);
    let role = extension.as_deref().map(detect_file_role);

    let mut body = stem.clone();
    let fansub = extract_leading_fansub(&mut body);
    let bracket_tokens = extract_enclosed_tokens(&body);

    let mut flags = ParseFlags::default();
    let mut release_version = extract_release_version(&body);
    let mut subtitles = SubtitleInfo::default();
    let mut technical = TechnicalInfo::default();
    let mut audio = AudioInfo::default();
    let mut raw_tags = Vec::new();
    let mut unparsed = Vec::new();

    for token in &bracket_tokens {
        if token.is_empty() {
            continue;
        }
        raw_tags.push(token.clone());
        update_flags(token, &mut flags);
        if release_version.is_none() {
            release_version = extract_release_version(token);
        }
        update_subtitle_info(token, &mut subtitles);
        update_technical_info(token, &mut technical, &mut audio);
    }

    if matches!(role, Some(FileRole::Subtitle)) {
        update_subtitle_info(&stem, &mut subtitles);
    }

    let season = parse_season(&body);
    let episode_range = parse_episode_range(&body, season.as_ref());
    if episode_range.is_some() {
        flags.is_batch = true;
    }
    let episode = if episode_range.is_none() {
        parse_single_episode(&body)
    } else {
        parse_dual_episode_alias(&body)
    };
    let title_body = cleanup_title_body(&body, episode_range.as_ref(), episode.as_ref());
    let titles = parse_titles(&title_body, season.as_ref());

    for token in bracket_tokens {
        if is_known_tag(&token) || parse_episode_range_token(&token).is_some() || token.is_empty() {
            continue;
        }
        unparsed.push(token);
    }

    ParseResult {
        source_kind,
        raw,
        file: FileInfo {
            file_name,
            extension,
            role,
        },
        fansub,
        titles,
        season,
        episode,
        episode_range,
        release_version,
        subtitles,
        technical,
        audio,
        flags,
        raw_tags,
        unparsed,
    }
}

fn basename(value: &str) -> String {
    value
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(value)
        .trim()
        .to_owned()
}

fn split_extension(file_name: &str) -> (String, Option<String>) {
    let lower = file_name.to_ascii_lowercase();
    if let Some(index) = lower.rfind('.') {
        let ext = lower[index + 1..].trim();
        if !ext.is_empty() && ext.len() <= 8 && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            return (file_name[..index].to_owned(), Some(ext.to_owned()));
        }
    }
    (file_name.to_owned(), None)
}

fn detect_file_role(extension: &str) -> FileRole {
    match extension {
        "mkv" | "mp4" | "avi" | "m2ts" | "ts" | "webm" | "mov" | "flv" => FileRole::Video,
        "ass" | "ssa" | "srt" | "sup" => FileRole::Subtitle,
        "ttf" | "otf" | "woff" | "woff2" => FileRole::FontPack,
        "7z" | "zip" | "rar" => FileRole::Archive,
        "flac" | "aac" | "mka" | "wav" => FileRole::Audio,
        _ => FileRole::Other,
    }
}

fn extract_leading_fansub(body: &mut String) -> FansubInfo {
    let Some(open) = body.chars().next() else {
        return FansubInfo::default();
    };
    let close = match open {
        '[' => ']',
        '【' => '】',
        _ => return FansubInfo::default(),
    };
    let Some(end) = body.find(close) else {
        return FansubInfo::default();
    };

    let content = body[open.len_utf8()..end].trim().to_owned();
    let remainder = body[end + close.len_utf8()..].trim_start().to_owned();
    *body = remainder;

    let groups = split_fansub_groups(&content);
    let primary = groups.first().cloned();
    let collaborators = groups.into_iter().skip(1).collect();
    FansubInfo {
        primary,
        collaborators,
    }
}

fn split_fansub_groups(value: &str) -> Vec<String> {
    let normalized = value.replace('＆', "&").replace('×', "&").replace('+', "&");
    normalized
        .split('&')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn extract_enclosed_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut stack = Vec::<(char, usize)>::new();

    for (index, character) in value.char_indices() {
        match character {
            '[' | '(' | '【' => stack.push((character, index)),
            ']' | ')' | '】' => {
                if let Some((open, start)) = stack.pop() {
                    if matches!((open, character), ('[', ']') | ('(', ')') | ('【', '】')) {
                        let content = value[start + open.len_utf8()..index].trim();
                        if !content.is_empty() {
                            tokens.push(content.to_owned());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    tokens
}

fn extract_release_version(value: &str) -> Option<u32> {
    version_regex()
        .captures_iter(value)
        .filter_map(|captures| captures.get(1))
        .filter_map(|item| item.as_str().parse::<u32>().ok())
        .last()
        .or_else(|| {
            compact_version_regex()
                .captures_iter(value)
                .filter_map(|captures| captures.get(1))
                .filter_map(|item| item.as_str().parse::<u32>().ok())
                .last()
        })
}

fn update_flags(token: &str, flags: &mut ParseFlags) {
    let lower = token.to_ascii_lowercase();
    if lower.contains("合集")
        || lower.contains("全集")
        || lower.contains("complete")
        || lower.contains("batch")
    {
        flags.is_batch = true;
    }
    if lower == "end" || lower == "fin" || lower.contains("[end]") || lower.contains("[fin]") {
        flags.is_complete = true;
    }
}

fn update_subtitle_info(token: &str, subtitles: &mut SubtitleInfo) {
    let raw = token.trim();
    if raw.is_empty() {
        return;
    }

    if subtitles.track_count.is_none() {
        subtitles.track_count = subtitle_track_regex()
            .captures(raw)
            .and_then(|captures| captures.get(1))
            .and_then(|item| item.as_str().parse::<u32>().ok());
    }
    if subtitles.raw_storage.is_none() {
        subtitles.raw_storage = detect_subtitle_storage_raw(raw);
    }
    if subtitles.storage.is_none() {
        subtitles.storage = detect_subtitle_storage(raw);
    }
    if subtitles.raw_language.is_none() {
        subtitles.raw_language = detect_language_raw(raw);
    }

    for language in detect_languages(raw) {
        if !subtitles.languages.iter().any(|item| item == &language) {
            subtitles.languages.push(language);
        }
    }
}

fn detect_subtitle_storage_raw(value: &str) -> Option<String> {
    if value.contains("内封") {
        return Some("内封字幕".to_owned());
    }
    if value.contains("外挂") || value.contains("外掛") {
        return Some("外挂字幕".to_owned());
    }
    if value.contains("内嵌") || value.contains("內嵌") {
        return Some("内嵌字幕".to_owned());
    }
    if value.contains("硬字幕") || value.contains("硬字") {
        return Some("硬字幕".to_owned());
    }
    None
}

fn detect_subtitle_storage(value: &str) -> Option<SubtitleStorage> {
    if value.contains("内封") {
        return Some(SubtitleStorage::Embedded);
    }
    if value.contains("外挂") || value.contains("外掛") {
        return Some(SubtitleStorage::External);
    }
    if value.contains("内嵌") || value.contains("內嵌") || value.contains("硬字幕") {
        return Some(SubtitleStorage::Hardcoded);
    }
    None
}

fn detect_language_raw(value: &str) -> Option<String> {
    let languages = detect_languages(value);
    if languages.is_empty() {
        return None;
    }
    Some(languages.join("+"))
}

fn detect_languages(value: &str) -> Vec<String> {
    let lower = value.to_ascii_lowercase();
    let mut languages = Vec::<String>::new();

    for (pattern, normalized) in [
        ("简繁英", ["zh-Hans", "zh-Hant", "en"].as_slice()),
        ("繁简英", ["zh-Hans", "zh-Hant", "en"].as_slice()),
        ("简繁日", ["zh-Hans", "zh-Hant", "ja"].as_slice()),
        ("繁日", ["zh-Hant", "ja"].as_slice()),
        ("简日", ["zh-Hans", "ja"].as_slice()),
        ("scen", ["zh-Hans", "en"].as_slice()),
        ("tcen", ["zh-Hant", "en"].as_slice()),
    ] {
        if lower.contains(pattern) {
            for item in normalized {
                push_language(&mut languages, item);
            }
        }
    }

    for (pattern, normalized) in [
        ("chs", "zh-Hans"),
        ("gb", "zh-Hans"),
        ("简体", "zh-Hans"),
        ("简中", "zh-Hans"),
        ("简", "zh-Hans"),
        ("cht", "zh-Hant"),
        ("big5", "zh-Hant"),
        ("繁体", "zh-Hant"),
        ("繁中", "zh-Hant"),
        ("繁", "zh-Hant"),
        ("eng", "en"),
        ("english", "en"),
        ("英", "en"),
        ("jpn", "ja"),
        ("jp", "ja"),
        ("日", "ja"),
    ] {
        if lower.contains(pattern) {
            push_language(&mut languages, normalized);
        }
    }

    languages
}

fn push_language(target: &mut Vec<String>, value: &str) {
    if !target.iter().any(|item| item == value) {
        target.push(value.to_owned());
    }
}

fn update_technical_info(token: &str, technical: &mut TechnicalInfo, audio: &mut AudioInfo) {
    let upper = token.to_ascii_uppercase();

    if technical.source.is_none() {
        technical.source = detect_source(&upper);
    }
    if technical.platform.is_none() {
        technical.platform = detect_platform(token);
    }
    if technical.resolution.is_none() {
        technical.resolution = detect_resolution(token);
    }
    if technical.video_codec.is_none() {
        technical.video_codec = detect_video_codec(token);
    }
    if technical.video_bit_depth.is_none() {
        technical.video_bit_depth = detect_bit_depth(token);
    }
    if audio.codec.is_none() {
        audio.codec = detect_audio_codec(token);
    }
}

fn detect_source(value: &str) -> Option<String> {
    for source in ["WEB-DL", "WEBRIP", "WEB-RIP", "WEB", "BDRIP", "BD"] {
        if value.contains(source) {
            return Some(
                match source {
                    "WEBRIP" | "WEB-RIP" => "WebRip",
                    "WEB-DL" => "WEB-DL",
                    "BDRIP" => "BDRip",
                    _ => source,
                }
                .to_owned(),
            );
        }
    }
    None
}

fn detect_platform(value: &str) -> Option<String> {
    for platform in [
        "Baha", "ABEMA", "B-Global", "CR", "MyVideo", "Ani-One", "ViuTV",
    ] {
        if value.contains(platform) {
            return Some(platform.to_owned());
        }
    }
    None
}

fn detect_resolution(value: &str) -> Option<String> {
    resolution_regex()
        .captures(value)
        .and_then(|captures| captures.get(1))
        .map(|item| item.as_str().to_owned())
}

fn detect_video_codec(value: &str) -> Option<String> {
    let upper = value.to_ascii_uppercase();
    for codec in ["HEVC-10BIT", "HEVC", "X265", "H265", "AVC", "X264", "H264"] {
        if upper.contains(codec) {
            return Some(codec.to_owned());
        }
    }
    None
}

fn detect_bit_depth(value: &str) -> Option<u8> {
    bit_depth_regex()
        .captures(value)
        .and_then(|captures| captures.get(1))
        .and_then(|item| item.as_str().parse::<u8>().ok())
}

fn detect_audio_codec(value: &str) -> Option<String> {
    let upper = value.to_ascii_uppercase();
    for codec in ["FLAC", "AAC", "OPUS", "EAC3", "AC3", "DDP"] {
        if upper.contains(codec) {
            return Some(codec.to_owned());
        }
    }
    None
}

fn parse_episode_range(body: &str, season: Option<&SeasonInfo>) -> Option<EpisodeRangeDescriptor> {
    let tokens = extract_enclosed_tokens(body);
    for token in tokens
        .iter()
        .filter(|token| dual_range_regex().is_match(token))
    {
        if let Some(range) = parse_episode_range_token(token) {
            return Some(range);
        }
    }
    for token in tokens
        .iter()
        .filter(|token| !dual_range_regex().is_match(token))
    {
        if let Some(range) = parse_episode_range_token(&token) {
            return Some(range);
        }
    }
    parse_episode_range_in_body(body, season)
}

fn parse_episode_range_in_body(
    body: &str,
    season: Option<&SeasonInfo>,
) -> Option<EpisodeRangeDescriptor> {
    if let Some(captures) = dual_range_regex().captures(body) {
        let primary_start = parse_episode_number(captures.get(1)?.as_str())?;
        let primary_end = parse_episode_number(captures.get(2)?.as_str())?;
        let secondary_start = parse_episode_number(captures.get(3)?.as_str())?;
        let secondary_end = parse_episode_number(captures.get(4)?.as_str())?;
        return Some(EpisodeRangeDescriptor {
            primary_start,
            primary_end,
            secondary_start: Some(secondary_start),
            secondary_end: Some(secondary_end),
        });
    }

    let captures = plain_range_regex().captures(body)?;
    let primary_start = parse_episode_number(captures.get(1)?.as_str())?;
    let primary_end = parse_episode_number(captures.get(2)?.as_str())?;
    if should_ignore_part_marker_plain_range(body, season, primary_start, primary_end) {
        return None;
    }

    Some(EpisodeRangeDescriptor {
        primary_start,
        primary_end,
        secondary_start: None,
        secondary_end: None,
    })
}

fn should_ignore_part_marker_plain_range(
    body: &str,
    season: Option<&SeasonInfo>,
    primary_start: EpisodeNumber,
    primary_end: EpisodeNumber,
) -> bool {
    let Some(season) = season else {
        return false;
    };
    let Some(part) = season.part else {
        return false;
    };
    if !part_regexes().iter().any(|regex| regex.is_match(body)) {
        return false;
    }
    if has_explicit_batch_hint(body) {
        return false;
    }

    primary_start.minor.is_none()
        && primary_end.minor.is_none()
        && primary_start.major == part
        && primary_end.major > primary_start.major
}

fn has_explicit_batch_hint(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("batch")
        || lower.contains("complete")
        || lower.contains("fin")
        || value.contains("鍚堥泦")
        || value.contains("鍏ㄩ泦")
}

fn parse_episode_range_token(value: &str) -> Option<EpisodeRangeDescriptor> {
    if let Some(captures) = dual_range_regex().captures(value) {
        let primary_start = parse_episode_number(captures.get(1)?.as_str())?;
        let primary_end = parse_episode_number(captures.get(2)?.as_str())?;
        let secondary_start = parse_episode_number(captures.get(3)?.as_str())?;
        let secondary_end = parse_episode_number(captures.get(4)?.as_str())?;
        return Some(EpisodeRangeDescriptor {
            primary_start,
            primary_end,
            secondary_start: Some(secondary_start),
            secondary_end: Some(secondary_end),
        });
    }

    if let Some(captures) = plain_range_regex().captures(value) {
        let primary_start = parse_episode_number(captures.get(1)?.as_str())?;
        let primary_end = parse_episode_number(captures.get(2)?.as_str())?;
        return Some(EpisodeRangeDescriptor {
            primary_start,
            primary_end,
            secondary_start: None,
            secondary_end: None,
        });
    }

    None
}

fn parse_single_episode(body: &str) -> Option<EpisodeDescriptor> {
    if let Some(captures) = dash_episode_regex().captures(body) {
        return Some(EpisodeDescriptor {
            primary: parse_episode_number(captures.get(2)?.as_str())?,
            secondary: captures
                .get(3)
                .and_then(|item| parse_episode_number(item.as_str())),
        });
    }

    if let Some(captures) = bracket_episode_regex().captures(body) {
        return Some(EpisodeDescriptor {
            primary: parse_episode_number(captures.get(2)?.as_str())?,
            secondary: None,
        });
    }

    None
}

fn parse_dual_episode_alias(body: &str) -> Option<EpisodeDescriptor> {
    dual_alias_regex().captures(body).and_then(|captures| {
        Some(EpisodeDescriptor {
            primary: parse_episode_number(captures.get(1)?.as_str())?,
            secondary: captures
                .get(2)
                .and_then(|item| parse_episode_number(item.as_str())),
        })
    })
}

fn parse_episode_number(value: &str) -> Option<EpisodeNumber> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (major, minor) = if let Some((major, minor)) = trimmed.split_once('.') {
        (major, Some(minor))
    } else {
        (trimmed, None)
    };

    let major = major.parse::<i64>().ok()?;
    let minor = minor.and_then(|value| {
        let digits = value.chars().take(1).collect::<String>();
        digits.parse::<u8>().ok()
    });

    Some(EpisodeNumber { major, minor })
}

fn parse_season(body: &str) -> Option<SeasonInfo> {
    let number = season_regexes()
        .iter()
        .find_map(|regex| regex.captures(body))
        .and_then(|captures| captures.get(1))
        .and_then(|item| parse_numeric_token(item.as_str()))?;
    let part = part_regexes()
        .iter()
        .find_map(|regex| regex.captures(body))
        .and_then(|captures| captures.get(1))
        .and_then(|item| parse_numeric_token(item.as_str()));

    Some(SeasonInfo { number, part })
}

fn parse_titles(body: &str, season: Option<&SeasonInfo>) -> TitleInfo {
    let normalized = body
        .replace('／', "/")
        .replace('｜', "|")
        .replace("  ", " ");
    let segments = normalized
        .split('/')
        .map(clean_title_segment)
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();

    let mut titles = TitleInfo::default();
    for segment in segments {
        let cleaned = strip_season_from_title(&segment, season);
        let cleaned = cleaned.trim().trim_end_matches('-').trim();
        if cleaned.is_empty() {
            continue;
        }

        if titles.primary.is_none() {
            titles.primary = Some(cleaned.to_owned());
        }

        let script = classify_script(cleaned);
        let candidate = TitleCandidate {
            text: cleaned.to_owned(),
            script,
        };

        match script {
            ScriptKind::Han if titles.cjk.is_none() => titles.cjk = Some(cleaned.to_owned()),
            ScriptKind::Japanese if titles.japanese.is_none() => {
                titles.japanese = Some(cleaned.to_owned())
            }
            ScriptKind::Latin if titles.latin.is_none() => titles.latin = Some(cleaned.to_owned()),
            _ => titles.alternates.push(candidate),
        }
    }

    titles
}

fn cleanup_title_body(
    body: &str,
    episode_range: Option<&EpisodeRangeDescriptor>,
    episode: Option<&EpisodeDescriptor>,
) -> String {
    let mut cleaned = enclosed_regex().replace_all(body, " ").into_owned();

    if episode_range.is_some() {
        cleaned = dual_range_regex().replace_all(&cleaned, " ").into_owned();
        cleaned = plain_range_regex().replace_all(&cleaned, " ").into_owned();
    }
    if let Some(episode) = episode {
        let primary = format_episode_label(episode.primary);
        cleaned = cleaned.replace(&format!("- {primary}"), " ");
        cleaned = cleaned.replace(&format!("-{primary}"), " ");
        cleaned = cleaned.replace(&format!("[{primary}]"), " ");
        if let Some(secondary) = episode.secondary {
            let dual = format!("{primary}({})", format_episode_label(secondary));
            cleaned = cleaned.replace(&dual, " ");
        }
    }

    cleaned = version_regex().replace_all(&cleaned, " ").into_owned();

    collapse_spaces(&cleaned)
}

fn clean_title_segment(value: &str) -> String {
    collapse_spaces(
        value
            .trim()
            .trim_matches(['-', '|', ':', '(', ')', '[', ']'])
            .trim(),
    )
}

fn strip_season_from_title(value: &str, season: Option<&SeasonInfo>) -> String {
    let mut cleaned = value.to_owned();

    for regex in season_regexes() {
        cleaned = regex.replace_all(&cleaned, " ").into_owned();
    }
    for regex in part_regexes() {
        cleaned = regex.replace_all(&cleaned, " ").into_owned();
    }

    if let Some(season) = season {
        cleaned = cleaned.replace(&format!("S{}", season.number), " ");
        cleaned = cleaned.replace(&format!("S{:02}", season.number), " ");
    }

    collapse_spaces(&cleaned)
}

fn classify_script(value: &str) -> ScriptKind {
    let mut has_han = false;
    let mut has_kana = false;
    let mut has_latin = false;

    for character in value.chars() {
        if ('\u{4E00}'..='\u{9FFF}').contains(&character) {
            has_han = true;
        } else if ('\u{3040}'..='\u{30FF}').contains(&character) {
            has_kana = true;
        } else if character.is_ascii_alphabetic() {
            has_latin = true;
        }
    }

    match (has_han, has_kana, has_latin) {
        (_, true, false) => ScriptKind::Japanese,
        (true, false, false) => ScriptKind::Han,
        (false, false, true) => ScriptKind::Latin,
        (false, false, false) => ScriptKind::Unknown,
        _ => ScriptKind::Mixed,
    }
}

fn collapse_spaces(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_numeric_token(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    trimmed
        .parse::<i64>()
        .ok()
        .or_else(|| chinese_number_to_i64(trimmed))
}

fn chinese_number_to_i64(value: &str) -> Option<i64> {
    match value {
        "一" => Some(1),
        "二" | "两" => Some(2),
        "三" => Some(3),
        "四" => Some(4),
        "五" => Some(5),
        "六" => Some(6),
        "七" => Some(7),
        "八" => Some(8),
        "九" => Some(9),
        "十" => Some(10),
        _ => None,
    }
}

fn format_episode_label(number: EpisodeNumber) -> String {
    match number.minor {
        Some(minor) => format!("{}.{}", number.major, minor),
        None => format!("{:02}", number.major),
    }
}

fn is_known_tag(token: &str) -> bool {
    detect_subtitle_storage(token).is_some()
        || !detect_languages(token).is_empty()
        || detect_source(&token.to_ascii_uppercase()).is_some()
        || detect_platform(token).is_some()
        || detect_resolution(token).is_some()
        || detect_video_codec(token).is_some()
        || detect_audio_codec(token).is_some()
        || extract_release_version(token).is_some()
        || token.eq_ignore_ascii_case("end")
        || token.eq_ignore_ascii_case("fin")
}

fn version_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\bv(?:er)?\s*([0-9]{1,2})\b").expect("valid version regex")
    })
}

fn subtitle_track_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(?:ASS|SRT)x([0-9]{1,2})\b").expect("valid subtitle track regex")
    })
}

fn compact_version_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\d{1,3}(?:\.\d+)?v([0-9]{1,2})").expect("valid compact version regex")
    })
}

fn resolution_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(3840x2160|1920x1080|1280x720|2160p|1080p|720p|540p|480p|4k)\b")
            .expect("valid resolution regex")
    })
}

fn bit_depth_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?i)\b([0-9]{1,2})bit\b").expect("valid bit depth regex"))
}

fn dual_range_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?ix)
            (\d{1,3}(?:\.\d+)?)
            \s*-\s*
            (\d{1,3}(?:\.\d+)?)
            \s*
            \(
                \s*(\d{1,3}(?:\.\d+)?)
                \s*-\s*
                (\d{1,3}(?:\.\d+)?)
            \s*\)
        ",
        )
        .expect("valid dual range regex")
    })
}

fn plain_range_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?ix)(\d{1,3}(?:\.\d+)?)\s*-\s*(\d{1,3}(?:\.\d+)?)(?:\s*(?:合集|全集|TV全集|batch|complete|fin))?")
            .expect("valid plain range regex")
    })
}

fn dash_episode_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?ix)
            ^(.*?)
            (?:^|[\s/])
            -\s*
            (\d{1,3}(?:\.\d+)?)
            (?:\((\d{1,3}(?:\.\d+)?)\))?
            (?:\s*v(?:er)?\s*\d{1,2})?
            (?:\s*(?:\[[^\]]*\]|\([^\)]*\)|\.[A-Za-z0-9]{2,8}))*\s*$
        ",
        )
        .expect("valid dash episode regex")
    })
}

fn bracket_episode_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?ix)
            ^(.*?)
            \[
            (\d{1,3}(?:\.\d+)?)
            \]
            (?:\s*(?:\[[^\]]*\]|\([^\)]*\)|\.[A-Za-z0-9]{2,8}))*\s*$
        ",
        )
        .expect("valid bracket episode regex")
    })
}

fn dual_alias_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?ix)\b(\d{1,3}(?:\.\d+)?)\s*\((\d{1,3}(?:\.\d+)?)\)")
            .expect("valid dual alias regex")
    })
}

fn enclosed_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"[\[\(【][^\]\)】]*[\]\)】]").expect("valid enclosed regex"))
}

fn season_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        vec![
            Regex::new(r"(?i)\bS(?:eason)?\s*0?(\d{1,2})\b").expect("valid season regex"),
            Regex::new(r"(?i)\b(\d{1,2})(?:st|nd|rd|th)\s+Season\b").expect("valid season regex"),
            Regex::new(r"第\s*([0-9]{1,2}|[一二三四五六七八九十两]+)\s*[季期]")
                .expect("valid season regex"),
        ]
    })
}

fn part_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        vec![
            Regex::new(r"(?i)\bPart\s*([0-9]{1,2})\b").expect("valid part regex"),
            Regex::new(r"第\s*([0-9]{1,2}|[一二三四五六七八九十两]+)\s*部分")
                .expect("valid part regex"),
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_file_name, parse_release_name};
    use crate::types::{FileRole, ParseResult, ParseSourceKind, ScriptKind, SubtitleStorage};

    fn print_case(name: &str, parsed: &ParseResult) {
        println!("===== {name} =====");
        println!(
            "{}",
            serde_json::to_string_pretty(parsed).expect("serialize parse result")
        );
    }

    #[test]
    fn parses_basic_release_with_version_and_subtitles() {
        let parsed = parse_release_name(
            "[LoliHouse] 关于我转生变成史莱姆这档事 第二季 / Tensei Shitara Slime Datta Ken 2nd Season - 02v2 [WebRip 1080p HEVC-10bit AAC][简繁英内封字幕]",
        );

        print_case("basic_release_with_version_and_subtitles", &parsed);
        assert_eq!(parsed.source_kind, ParseSourceKind::ReleaseTitle);
        assert_eq!(parsed.fansub.primary.as_deref(), Some("LoliHouse"));
        assert_eq!(
            parsed.titles.cjk.as_deref(),
            Some("关于我转生变成史莱姆这档事")
        );
        assert_eq!(
            parsed.titles.latin.as_deref(),
            Some("Tensei Shitara Slime Datta Ken")
        );
        assert_eq!(parsed.season.map(|item| item.number), Some(2));
        assert_eq!(parsed.episode.map(|item| item.primary.major), Some(2));
        assert_eq!(parsed.release_version, Some(2));
        assert_eq!(parsed.subtitles.storage, Some(SubtitleStorage::Embedded));
        assert_eq!(parsed.subtitles.languages, vec!["zh-Hans", "zh-Hant", "en"]);
        assert_eq!(parsed.technical.source.as_deref(), Some("WebRip"));
        assert_eq!(parsed.technical.resolution.as_deref(), Some("1080p"));
        assert_eq!(parsed.audio.codec.as_deref(), Some("AAC"));
    }

    #[test]
    fn parses_collaborators_and_file_extension() {
        let parsed = parse_file_name(
            "[Nekomoe kissaten&LoliHouse] Sousou no Frieren - 01 [WebRip 1080p HEVC-10bit AAC ASSx2].mkv",
        );

        print_case("collaborators_and_file_extension", &parsed);
        assert_eq!(parsed.source_kind, ParseSourceKind::FileName);
        assert_eq!(parsed.fansub.primary.as_deref(), Some("Nekomoe kissaten"));
        assert_eq!(parsed.fansub.collaborators, vec!["LoliHouse"]);
        assert_eq!(parsed.file.extension.as_deref(), Some("mkv"));
        assert_eq!(parsed.file.role, Some(FileRole::Video));
        assert_eq!(parsed.episode.map(|item| item.primary.major), Some(1));
        assert_eq!(parsed.subtitles.track_count, Some(2));
    }

    #[test]
    fn parses_dual_episode_notation() {
        let parsed = parse_release_name(
            "[LoliHouse] 关于我转生变成史莱姆这档事 第三季 / Tensei Shitara Slime Datta Ken 3rd Season - 24(72) [WebRip 1080p HEVC-10bit AAC][简繁内封字幕][END]",
        );

        print_case("dual_episode_notation", &parsed);
        let episode = parsed.episode.expect("episode");
        assert_eq!(episode.primary.major, 24);
        assert_eq!(episode.secondary.expect("secondary").major, 72);
        assert_eq!(parsed.season.map(|item| item.number), Some(3));
        assert!(parsed.flags.is_complete);
    }

    #[test]
    fn parses_fractional_episode_alias() {
        let parsed = parse_release_name(
            "[LoliHouse] 关于我转生变成史莱姆这档事 第三季 / Tensei Shitara Slime Datta Ken 3rd Season - 00(48.5) [WebRip 1080p HEVC-10bit AAC][简繁内封字幕]",
        );

        print_case("fractional_episode_alias", &parsed);
        let episode = parsed.episode.expect("episode");
        assert_eq!(episode.primary.major, 0);
        let secondary = episode.secondary.expect("secondary");
        assert_eq!(secondary.major, 48);
        assert_eq!(secondary.minor, Some(5));
    }

    #[test]
    fn parses_dual_batch_range() {
        let parsed = parse_release_name(
            "[LoliHouse] 关于我转生变成史莱姆这档事 第三季 / Tensei Shitara Slime Datta Ken 3rd Season [48.5-72(00-24) 合集][WebRip 1080p HEVC-10bit AAC][简繁内封字幕][Fin]",
        );

        print_case("dual_batch_range", &parsed);
        let range = parsed.episode_range.expect("range");
        assert_eq!(range.primary_start.major, 48);
        assert_eq!(range.primary_start.minor, Some(5));
        assert_eq!(range.primary_end.major, 72);
        assert_eq!(range.secondary_start.expect("secondary").major, 0);
        assert_eq!(range.secondary_end.expect("secondary").major, 24);
        assert!(parsed.flags.is_batch);
        assert!(parsed.flags.is_complete);
    }

    #[test]
    fn parses_part_two_batch_range() {
        let parsed = parse_release_name(
            "[LoliHouse] 关于我转生变成史莱姆这档事 第二季 第2部分 / Tensei Shitara Slime Datta Ken S2 Part 2 [37-48合集][WebRip 1080p HEVC-10bit AAC][简繁英内封]",
        );

        print_case("part_two_batch_range", &parsed);
        let range = parsed.episode_range.expect("range");
        assert_eq!(range.primary_start.major, 37);
        assert_eq!(range.primary_end.major, 48);
        assert_eq!(parsed.season.map(|item| item.number), Some(2));
        assert_eq!(parsed.season.and_then(|item| item.part), Some(2));
        assert!(parsed.flags.is_batch);
    }

    #[test]
    fn does_not_treat_part_marker_episode_as_batch_range() {
        let parsed = parse_release_name(
            "[Up to 21°C] Mushoku Tensei S2 Part 2 - 24 [WebRip 1080p HEVC-10bit AAC]",
        );

        print_case("part_two_single_episode", &parsed);
        assert!(parsed.episode_range.is_none());
        assert_eq!(parsed.season.map(|item| item.number), Some(2));
        assert_eq!(parsed.season.and_then(|item| item.part), Some(2));
        assert_eq!(parsed.episode.expect("episode").primary.major, 24);
    }

    #[test]
    fn parses_subtitle_file_language_hint() {
        let parsed = parse_file_name(
            "[BeanSub&FZSD&LoliHouse] Jujutsu Kaisen - 59 [WebRip 1080p HEVC-10bit AAC ASSx2].CHS.ass",
        );

        print_case("subtitle_file_language_hint", &parsed);
        assert_eq!(parsed.file.role, Some(FileRole::Subtitle));
        assert_eq!(parsed.file.extension.as_deref(), Some("ass"));
        assert_eq!(parsed.subtitles.languages, vec!["zh-Hans"]);
    }

    #[test]
    fn classifies_script_candidates() {
        let parsed = parse_release_name(
            "[LoliHouse] 关于我转生变成史莱姆这档事 / 転生したらスライムだった件 / Tensei Shitara Slime Datta Ken - 01 [WebRip 1080p]",
        );

        print_case("script_candidates", &parsed);
        assert_eq!(
            parsed.titles.cjk.as_deref(),
            Some("关于我转生变成史莱姆这档事")
        );
        assert_eq!(
            parsed.titles.latin.as_deref(),
            Some("Tensei Shitara Slime Datta Ken")
        );
        assert_eq!(
            parsed.titles.japanese.as_deref(),
            Some("転生したらスライムだった件")
        );
        assert!(
            !parsed
                .titles
                .alternates
                .iter()
                .any(|candidate| candidate.script == ScriptKind::Japanese)
        );
    }
}
