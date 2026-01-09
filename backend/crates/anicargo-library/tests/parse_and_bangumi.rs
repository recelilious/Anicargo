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
    println!("parsed media: {:?}", parsed);
    assert!(parsed.parse_ok, "expected filename parse to succeed");

    let title = parsed.title.as_deref().unwrap_or("");
    println!("parsed title: {}", title);
    let normalized = normalize_title(title);
    assert_eq!(normalized, "spyxfamily", "unexpected title: {}", title);

    let episode = parsed.episode.as_deref().unwrap_or("");
    println!("parsed episode: {}", episode);
    assert_eq!(episode, "12", "unexpected episode: {}", episode);

    let year = parsed.year.as_deref().unwrap_or("");
    println!("parsed year: {}", year);
    assert_eq!(year, "2025", "unexpected year: {}", year);

    let release_group = parsed.release_group.as_deref().unwrap_or("");
    println!("parsed release group: {}", release_group);
    assert_eq!(release_group, "Sakurato", "unexpected group: {}", release_group);

    let resolution = parsed.resolution.as_deref().unwrap_or("");
    println!("parsed resolution: {}", resolution);
    assert_eq!(resolution, "1080p", "unexpected resolution: {}", resolution);
}

#[tokio::test]
async fn searches_bangumi_for_title() {
    if std::env::var("ANICARGO_BANGUMI_TEST").ok().as_deref() != Some("1") {
        println!("skipping bangumi search (set ANICARGO_BANGUMI_TEST=1 to enable)");
        return;
    }

    let parsed = parse_filename(SAMPLE_FILE);
    let title = parsed.title.as_deref().unwrap_or("Spy x Family");
    let client = BangumiClient::new(None, "Anicargo-test/0.1".to_string())
        .expect("bangumi client");

    let result = tokio::time::timeout(Duration::from_secs(15), client.search_anime(title, 10))
        .await
        .expect("bangumi request timed out")
        .expect("bangumi search failed");

    println!("bangumi results: {}", result.data.len());
    for (idx, subject) in result.data.iter().take(5).enumerate() {
        println!(
            "result[{}]: id={} name={} name_cn={}",
            idx, subject.id, subject.name, subject.name_cn
        );
    }
    assert!(!result.data.is_empty(), "bangumi search returned no results");

    let has_spy_family = result.data.iter().any(|subject| {
        let name = subject.name.to_lowercase();
        name.contains("spy") && name.contains("family")
    });
    assert!(has_spy_family, "expected SPY FAMILY result in bangumi search");
}
