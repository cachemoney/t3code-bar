use bar_usage::{decode_cache, CacheError, Period, Window};
use chrono::{TimeZone, Utc};
use serde_json::json;

fn doc(version: i64, files: &str) -> String {
    format!(
        r#"{{"version":{version},"sources":{{"claude\u0000/h":{{"dir":"/h"}}}},"models":["m"],"sessions":["s"],"files":{{{files}}}}}"#
    )
}
#[test]
fn versions_bad_input_and_file_rejection() {
    for version in 4..=11 {
        assert!(decode_cache(doc(version, "").as_bytes()).is_ok());
    }
    assert_eq!(
        decode_cache(doc(12, "").as_bytes()).unwrap_err(),
        CacheError::UnsupportedVersion(Some(12))
    );
    assert_eq!(
        decode_cache(b"{\"version\":11").unwrap_err(),
        CacheError::Unreadable
    );
    let files = r#""/h/bad":{"p":"claude","m":1,"r":[[1,0]]},"/h/good":{"p":"claude","m":1,"r":[[1,0,0,1,2,3,4,5,null,null,0]]}"#;
    let data = decode_cache(doc(11, files).as_bytes()).unwrap();
    assert_eq!(data.rejected_files, 1);
    assert_eq!(data.files[0].records[0].processed(), 10);
}
#[test]
fn cache_order_controls_first_wins_even_before_window() {
    let now = Utc.with_ymd_and_hms(2026, 11, 6, 9, 8, 0).unwrap();
    let inside = now.timestamp_millis() - 1000;
    let outside = inside - 2 * 86_400_000;
    let files = format!(
        r#""/h/z":{{"p":"claude","m":{inside},"r":[[{outside},0,0,1,0,0,0,0,"same",null,0]]}},"/h/a":{{"p":"claude","m":{inside},"r":[[{inside},0,0,2,0,0,0,0,"same",null,0]]}}"#
    );
    let data = decode_cache(doc(11, &files).as_bytes()).unwrap();
    assert_eq!(
        data.files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        ["/h/z", "/h/a"]
    );
    let mut records = 0;
    let selection = data.select(
        &Window::new(Period::Day, now, chrono_tz::Asia::Ho_Chi_Minh),
        |_, _, _, _| records += 1,
    );
    assert_eq!(
        (
            records,
            selection.duplicates_dropped,
            selection.out_of_window
        ),
        (0, 1, 1)
    );
}
#[test]
fn canary_selection_matches_oracle_counts_for_all_periods() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/usage");
    let dataset =
        decode_cache(&std::fs::read(root.join("state/usage-scan-cache-v11.json")).unwrap())
            .unwrap();
    let golden: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("golden.json")).unwrap()).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 11, 6, 9, 8, 0).unwrap();
    for (period, label) in [
        (Period::Day, "Past 24h"),
        (Period::Week, "7 days"),
        (Period::Month, "30 days"),
        (Period::Quarter, "90 days"),
    ] {
        let mut count = 0;
        let mut tokens = 0;
        let result = dataset.select(
            &Window::new(period, now, chrono_tz::Asia::Ho_Chi_Minh),
            |_, _, r, _| {
                count += 1;
                tokens += r.processed();
            },
        );
        let expected = &golden["periods"][label];
        assert_eq!(json!(count), expected["records"], "{label} records");
        assert_eq!(json!(tokens), expected["totalTokens"], "{label} tokens");
        assert_eq!(
            json!(result.sessions()),
            expected["sessions"],
            "{label} sessions"
        );
        assert_eq!(
            json!(result.duplicates_dropped),
            expected["duplicatesDropped"],
            "{label} duplicates"
        );
    }
}

#[test]
fn deferred_invalid_rows_reject_only_the_file_and_integral_float_counts_survive() {
    let data = decode_cache(br#"{"version":11,"files":{"missing-provider":{"r":0},"valid":{"p":"claude","m":1,"mr":1.0,"r":[]}}}"#).unwrap();
    assert_eq!(data.rejected_files, 1);
    assert_eq!(data.files.len(), 1);
    assert_eq!(data.files[0].malformed, 1);
}

#[test]
fn cache_eager_rows_fail_document_and_deferred_rows_require_file_header() {
    use bar_usage::CacheError;
    let eager = br#"{"version":11,"files":{"bad":{"p":"codex","r":[3]}}}"#;
    assert!(matches!(decode_cache(eager), Err(CacheError::Unreadable)));
    let deferred_without_header = br#"{"version":11,"files":{"bad":{"r":[3]}}}"#;
    assert_eq!(
        decode_cache(deferred_without_header)
            .unwrap()
            .rejected_files,
        1
    );
    let deferred_with_header = br#"{"version":11,"files":{"bad":{"r":[3],"p":"codex","m":1}}}"#;
    assert!(matches!(
        decode_cache(deferred_with_header),
        Err(CacheError::Unreadable)
    ));
}
