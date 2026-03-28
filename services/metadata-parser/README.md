# Anicargo Metadata Parser

The metadata parser is a standalone Rust library for parsing anime release
titles and file names into a stable structured representation.

This crate is intentionally independent from AnimeGarden-specific payload types
and from the Anicargo backend. It focuses on turning raw strings into a stable,
deterministic parse result that the backend can later enrich or correct with
provider metadata.

Current capabilities include:

- leading fansub extraction and collaborator splitting
- CJK, Japanese, and Latin title candidate detection
- season, part, single-episode, dual-number, fractional, and range parsing
- release revision parsing such as `v2`
- subtitle language, subtitle storage, and subtitle track-count detection
- source, platform, resolution, video codec, bit-depth, and audio codec parsing
- file extension and file-role classification
- raw tag preservation and unparsed token carry-through

## Quick Start

```rust
use anicargo_metadata_parser::{parse_file_name, parse_release_name};

let release = parse_release_name(
    "[LoliHouse] Tensei Shitara Slime Datta Ken 3rd Season - 24(72) [WebRip 1080p HEVC-10bit AAC][CHS CHT][END]",
);
let file = parse_file_name(
    "[Nekomoe kissaten&LoliHouse] Sousou no Frieren - 01 [WebRip 1080p HEVC-10bit AAC ASSx2].mkv",
);

assert_eq!(release.season.map(|item| item.number), Some(3));
assert_eq!(file.file.extension.as_deref(), Some("mkv"));
```

Run the test suite:

```powershell
cargo test --manifest-path .\services\metadata-parser\Cargo.toml
```

Run tests with pretty JSON output for each regression sample:

```powershell
cargo test --manifest-path .\services\metadata-parser\Cargo.toml -- --nocapture --test-threads=1
```

## Documentation

- [Architecture guide](./docs/ARCHITECTURE.md)
- [Data model reference](./docs/DATA_MODEL.md)
- [Parsing rules](./docs/PARSING_RULES.md)
- [Testing guide](./docs/TESTING.md)
