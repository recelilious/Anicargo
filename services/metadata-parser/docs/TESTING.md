# Metadata Parser Testing Guide

## 1. Run The Test Suite

Standard test run:

```powershell
cargo test --manifest-path .\services\metadata-parser\Cargo.toml
```

Test run with readable JSON output for each regression sample:

```powershell
cargo test --manifest-path .\services\metadata-parser\Cargo.toml -- --nocapture --test-threads=1
```

The single-thread option keeps the pretty-printed output in a predictable order.

## 2. Current Regression Coverage

The initial suite covers:

- a standard release title with season, episode, revision, subtitle, and codec tags
- collaborator fansub parsing plus file extension detection
- dual episode notation such as `24(72)`
- fractional alias notation such as `00(48.5)`
- dual batch ranges such as `48.5-72(00-24)`
- season-part batch ranges such as `37-48` for part 2
- subtitle file names with language hints such as `.CHS.ass`
- mixed-script title segmentation into Han, Japanese, and Latin candidates

## 3. Adding A New Regression Case

Preferred workflow:

1. add a failing test in `services/metadata-parser/src/parser.rs`
2. use a real-world sample string when possible
3. assert only the fields that matter for the new rule
4. keep the pretty JSON output so the full parse remains easy to inspect
5. rerun the suite with `--nocapture --test-threads=1`

## 4. What To Watch For

When reviewing a new parse result, pay extra attention to:

- whether titles lost important text during cleanup
- whether a secondary episode number was incorrectly dropped
- whether a range was mistaken for a single episode
- whether subtitle languages or storage were over-inferred
- whether known bracket tags leaked into `unparsed`

## 5. Maintenance Notes

This crate is deterministic and rule-based, so regression tests are the main
safety net. Whenever a new naming convention is added, it should arrive with a
focused test case.
