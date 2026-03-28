# Metadata Parser Architecture

## 1. Entry Points

Public library exports:

- `services/metadata-parser/src/lib.rs`

Core parsing implementation:

- `services/metadata-parser/src/parser.rs`

Shared public types:

- `services/metadata-parser/src/types.rs`

## 2. Library Boundary

The metadata parser is library-only.

It does not expose:

- an HTTP surface
- a database model
- provider-specific request types
- a direct dependency on AnimeGarden, Bangumi, or backend runtime state

This boundary is intentional. The crate turns raw strings into structured parse
results, while provider-aware corrections remain the responsibility of the
consumer.

## 3. Processing Pipeline

Both `parse_release_name` and `parse_file_name` share the same rule pipeline.
The only difference is the source kind and the initial file-name handling.

Current pipeline stages:

1. Normalize the input string and determine `ParseSourceKind`.
2. Split the basename and optional file extension when the source is a file name.
3. Extract the leading fansub segment such as `[LoliHouse]`.
4. Collect bracketed tokens for later subtitle, technical, and batch detection.
5. Parse release revision, subtitle hints, technical tags, and file role.
6. Parse episode range, single-episode notation, and dual-number aliases.
7. Parse season and part hints.
8. Remove already-consumed fragments and classify title candidates by script.
9. Assemble `ParseResult`, preserving both recognized and unresolved tokens.

## 4. Output Strategy

The crate prefers explicit structure over implicit normalization.

Important design choices:

- dual numbering is preserved instead of collapsed
- fractional episodes are represented as `major + minor`
- batch ranges keep both primary and secondary ranges when present
- raw tags are preserved even when a normalized field is also extracted
- unresolved tokens are carried in `unparsed` instead of being silently dropped

This makes the parser easier to audit and safer to integrate into later
provider-aware correction layers.

## 5. Separation Of Concerns

The parser is responsible for:

- extracting deterministic fields from a single raw string
- handling common anime release naming conventions
- returning a stable Rust data model for downstream consumers

The parser is not responsible for:

- canonical title matching against Bangumi or another provider
- remapping absolute episode numbers into season-local numbers
- deciding which provider field should override a parsed field
- interpreting download policy or ranking candidates

Those concerns belong in the Anicargo backend integration layer.

## 6. Extension Strategy

When adding support for a new pattern, prefer this order:

1. add a focused regression test
2. add or refine the narrowest rule that fixes the case
3. preserve existing `ParseResult` shape when possible
4. keep provider-specific corrections outside this crate

This keeps the parser deterministic, explainable, and reusable.
