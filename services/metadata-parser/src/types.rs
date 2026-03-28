use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseSourceKind {
    ReleaseTitle,
    FileName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRole {
    Video,
    Subtitle,
    FontPack,
    Archive,
    Audio,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptKind {
    Han,
    Latin,
    Japanese,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleStorage {
    Embedded,
    External,
    Hardcoded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FansubInfo {
    pub primary: Option<String>,
    #[serde(default)]
    pub collaborators: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleCandidate {
    pub text: String,
    pub script: ScriptKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TitleInfo {
    pub primary: Option<String>,
    pub cjk: Option<String>,
    pub latin: Option<String>,
    pub japanese: Option<String>,
    #[serde(default)]
    pub alternates: Vec<TitleCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonInfo {
    pub number: i64,
    pub part: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeNumber {
    pub major: i64,
    pub minor: Option<u8>,
}

impl EpisodeNumber {
    pub fn decimal(self) -> f64 {
        match self.minor {
            Some(minor) => self.major as f64 + (minor as f64 / 10.0),
            None => self.major as f64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeDescriptor {
    pub primary: EpisodeNumber,
    pub secondary: Option<EpisodeNumber>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeRangeDescriptor {
    pub primary_start: EpisodeNumber,
    pub primary_end: EpisodeNumber,
    pub secondary_start: Option<EpisodeNumber>,
    pub secondary_end: Option<EpisodeNumber>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SubtitleInfo {
    pub raw_language: Option<String>,
    pub raw_storage: Option<String>,
    pub storage: Option<SubtitleStorage>,
    #[serde(default)]
    pub languages: Vec<String>,
    pub track_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TechnicalInfo {
    pub source: Option<String>,
    pub platform: Option<String>,
    pub resolution: Option<String>,
    pub video_codec: Option<String>,
    pub video_bit_depth: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AudioInfo {
    pub codec: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInfo {
    pub file_name: String,
    pub extension: Option<String>,
    pub role: Option<FileRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ParseFlags {
    pub is_batch: bool,
    pub is_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseResult {
    pub source_kind: ParseSourceKind,
    pub raw: String,
    pub file: FileInfo,
    pub fansub: FansubInfo,
    pub titles: TitleInfo,
    pub season: Option<SeasonInfo>,
    pub episode: Option<EpisodeDescriptor>,
    pub episode_range: Option<EpisodeRangeDescriptor>,
    pub release_version: Option<u32>,
    pub subtitles: SubtitleInfo,
    pub technical: TechnicalInfo,
    pub audio: AudioInfo,
    pub flags: ParseFlags,
    #[serde(default)]
    pub raw_tags: Vec<String>,
    #[serde(default)]
    pub unparsed: Vec<String>,
}
