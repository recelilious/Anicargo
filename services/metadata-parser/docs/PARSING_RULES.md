# Metadata Parser Parsing Rules

## 1. Leading Fansub Extraction

The parser first checks for a leading group block such as:

- `[LoliHouse]`
- `[BeanSub&FZSD&LoliHouse]`

This block is split into one primary group plus zero or more collaborators.

## 2. Bracketed Token Collection

Bracketed segments are collected before title cleanup so the parser can inspect
tokens such as:

- subtitle hints
- source and codec tags
- completion markers
- batch ranges

These tokens are preserved in `raw_tags` even when normalized fields are also
extracted from them.

## 3. Title Segmentation

The title body is split mainly by slash separators after the parser removes
episode numbers, versions, and consumed bracket tags.

Each surviving segment is classified into one of these script buckets:

- Han
- Japanese
- Latin
- Mixed
- Unknown

The parser does not force a canonical title. It simply records the best
available candidates.

## 4. Season And Part Detection

Current season patterns include forms such as:

- `S2`
- `Season 2`
- `2nd Season`
- Chinese ordinal season markers

Current part patterns include forms such as:

- `Part 2`
- Chinese part markers

When detected, these values are removed from title candidates and stored in
`SeasonInfo`.

## 5. Episode Detection

Current single-episode patterns include forms such as:

- `- 01`
- `- 02v2`
- `[01]`
- `24(72)`
- `00(48.5)`

Current range patterns include forms such as:

- `37-48`
- `48.5-72(00-24)`

Parsing rules prefer explicit dual ranges before plain ranges so mixed absolute
and season-local batch forms are preserved correctly.

## 6. Release Revision Detection

The parser currently detects revisions such as:

- `v2`
- `ver2`
- compact forms such as `02v2`

The normalized field is `release_version`.

## 7. Subtitle Inference

Subtitle parsing currently looks for:

- language combinations
- embedded vs external vs hardcoded subtitle hints
- subtitle track counts such as `ASSx2`
- file-name language hints such as `.CHS.ass`

The parser keeps both normalized values and raw inference strings.

## 8. Technical Tag Inference

Technical parsing currently looks for:

- source tags such as `WebRip` and `WEB-DL`
- known platforms such as `Baha`
- resolution values such as `1080p`
- video codec hints such as `HEVC` and `x264`
- bit depth such as `10bit`
- audio codec hints such as `AAC` and `FLAC`

## 9. File Role Inference

File role is inferred from extension.

Current categories include:

- video containers
- subtitle files
- archives
- font packs
- audio-only files

## 10. Known Limits

The parser intentionally stops short of provider-aware correction.

Examples of work that should happen outside this crate:

- canonical title resolution against Bangumi
- deciding whether `72` should be remapped to season-local episode `24`
- reconciling parser output with AnimeGarden structured metadata
- ranking conflicting interpretations using subject context

When the parser cannot fully normalize a token, it preserves the fragment in
`raw_tags` or `unparsed` so downstream code can still inspect it.
