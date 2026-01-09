use anicargo_bangumi::BangumiClient;
use anicargo_library::parse_filename;
use std::time::Duration;

const SAMPLE_FILE: &str =
    "[Sakurato] Spy x Family (2025) [12][AVC-8 bit 1080p ACC][CHT].mp4";

fn normalize_title(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

#[test]
fn parses_filename_with_anitomy() {
    let parsed = parse_filename(SAMPLE_FILE);
    assert!(parsed.parse_ok, "expected filename parse to succeed");

    let title = parsed.title.as_deref().unwrap_or("");
    let normalized = normalize_title(title);
    assert_eq!(normalized, "spyxfamily", "unexpected title: {}", title);

    let episode = parsed.episode.as_deref().unwrap_or("");
    assert_eq!(episode, "12", "unexpected episode: {}", episode);
}

#[tokio::test]
async fn searches_bangumi_for_title() {
    let parsed = parse_filename(SAMPLE_FILE);
    let title = parsed.title.as_deref().unwrap_or("Spy x Family");
    let client = BangumiClient::new(None, "Anicargo-test/0.1".to_string())
        .expect("bangumi client");

    let result = tokio::time::timeout(Duration::from_secs(15), client.search_anime(title, 10))
        .await
        .expect("bangumi request timed out")
        .expect("bangumi search failed");

    assert!(!result.data.is_empty(), "bangumi search returned no results");

    let has_spy_family = result.data.iter().any(|subject| {
        let name = subject.name.to_lowercase();
        name.contains("spy") && name.contains("family")
    });
    assert!(has_spy_family, "expected SPY FAMILY result in bangumi search");
}
