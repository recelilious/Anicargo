# Metadata Parser Data Model

## 1. Public Functions

The crate currently exposes two entry points:

- `parse_release_name(input: &str) -> ParseResult`
- `parse_file_name(input: &str) -> ParseResult`

Both functions return the same `ParseResult` structure.

## 2. Top-Level Result

`ParseResult` contains the full structured output for a single string.

Top-level fields:

- `source_kind`: whether the input was treated as a release title or a file name
- `raw`: the original input string after trimming
- `file`: file-name-derived data such as extension and file role
- `fansub`: leading release-group information
- `titles`: parsed title candidates
- `season`: detected season and optional part number
- `episode`: detected single-episode descriptor
- `episode_range`: detected batch range descriptor
- `release_version`: release revision such as `v2`
- `subtitles`: subtitle language and storage hints
- `technical`: source, platform, resolution, and video details
- `audio`: audio codec details
- `flags`: batch and completion markers
- `raw_tags`: bracketed tokens preserved from the original input
- `unparsed`: tokens that were not matched by known rules

Consumers should treat most fields as optional.

## 3. Fansub Model

`FansubInfo` separates one leading group block into:

- `primary`
- `collaborators`

Example:

- input: `[Nekomoe kissaten&LoliHouse]`
- output: `primary = "Nekomoe kissaten"`, `collaborators = ["LoliHouse"]`

## 4. Title Model

`TitleInfo` keeps multiple views of the title:

- `primary`: first surviving title segment after cleanup
- `cjk`: Han-script title candidate
- `latin`: Latin-script title candidate
- `japanese`: Kana-bearing Japanese title candidate
- `alternates`: additional title candidates with explicit `ScriptKind`

This model is intentionally permissive. Title canonicalization across providers
belongs in a higher-level integration layer.

## 5. Season And Episode Model

`SeasonInfo` stores:

- `number`
- `part`

`EpisodeNumber` stores:

- `major`
- `minor`

The `minor` field is used for fractional episodes such as `48.5`.

`EpisodeDescriptor` stores:

- `primary`
- `secondary`

This allows the parser to keep dual numbering formats such as `24(72)` without
choosing one numbering scheme too early.

`EpisodeRangeDescriptor` stores:

- `primary_start`
- `primary_end`
- `secondary_start`
- `secondary_end`

This allows batch notations such as `48.5-72(00-24)` to keep both absolute and
season-local ranges.

## 6. Subtitle Model

`SubtitleInfo` stores:

- `raw_language`
- `raw_storage`
- `storage`
- `languages`
- `track_count`

The raw fields preserve the original inference source, while the normalized
fields expose a stable integration shape.

Current normalized language examples:

- `zh-Hans`
- `zh-Hant`
- `en`
- `ja`

Current storage examples:

- `embedded`
- `external`
- `hardcoded`

## 7. Technical And File Model

`TechnicalInfo` stores:

- `source`
- `platform`
- `resolution`
- `video_codec`
- `video_bit_depth`

`AudioInfo` currently stores:

- `codec`

`FileInfo` stores:

- `file_name`
- `extension`
- `role`

Current file roles include:

- `video`
- `subtitle`
- `font_pack`
- `archive`
- `audio`
- `other`

## 8. Flags And Preservation Fields

`ParseFlags` stores:

- `is_batch`
- `is_complete`

The parser also preserves:

- `raw_tags` for bracketed tokens that may still be useful downstream
- `unparsed` for unresolved fragments that should remain visible during review

These fields are important for safe iteration because they prevent silent data
loss when a new naming convention is encountered.
