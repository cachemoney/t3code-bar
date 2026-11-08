use bar_limits::{LimitsReader, LimitsState};
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::{fs, path::PathBuf};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}
#[test]
fn swift_fixture_export_matches() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("caches")).unwrap();
    fs::copy(
        root().join("limits/codex.json"),
        temp.path().join("caches/codex.json"),
    )
    .unwrap();
    let now = Utc.with_ymd_and_hms(2026, 11, 6, 10, 0, 0).unwrap();
    let s = LimitsReader::new()
        .read(&temp.path().join("state"), now)
        .unwrap();
    let mut actual = s.fixture_export(now);
    let expected: Value =
        serde_json::from_slice(&fs::read(root().join("expected/limits.json")).unwrap()).unwrap();
    fn normalize(v: &mut Value) {
        match v {
            Value::Number(n) => *n = serde_json::Number::from_f64(n.as_f64().unwrap()).unwrap(),
            Value::Array(a) => a.iter_mut().for_each(normalize),
            Value::Object(o) => o.values_mut().for_each(normalize),
            _ => {}
        }
    }
    let mut expected = expected;
    normalize(&mut actual);
    normalize(&mut expected);
    assert_eq!(actual, expected);
}
#[test]
fn retains_last_valid_file_and_drops_deleted() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join("caches");
    fs::create_dir(&d).unwrap();
    let p = d.join("x.json");
    fs::copy(root().join("limits/codex.json"), &p).unwrap();
    let mut r = LimitsReader::new();
    let now = Utc::now();
    assert_eq!(
        r.read(&t.path().join("state"), now).unwrap().accounts.len(),
        1
    );
    fs::write(&p, "{").unwrap();
    assert_eq!(
        r.read(&t.path().join("state"), now).unwrap().accounts.len(),
        1
    );
    fs::remove_file(p).unwrap();
    assert_eq!(
        r.read(&t.path().join("state"), now).unwrap().state,
        LimitsState::NoLimitsReported
    );
}

#[test]
fn merge_respects_workspace_and_latest_present_plan() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join("caches");
    fs::create_dir(&d).unwrap();
    for (name, time, workspace, extra) in [
        ("a", "2026-01-01T00:00:00Z", "one", r#", "label":"Old""#),
        ("b", "2026-01-03T00:00:00Z", "one", ""),
        ("c", "2026-01-02T00:00:00Z", "one", r#", "label":"New""#),
        ("d", "2026-01-01T00:00:00Z", "two", ""),
    ] {
        let data = format!(
            r#"{{"instanceId":"{name}","driver":"codex","enabled":true,"installed":true,"auth":{{"email":"me@example.org","workspaceId":"{workspace}"{extra}}},"usageLimits":{{"checkedAt":"{time}","windows":[{{"id":"w","kind":"weekly","usedPercent":20}}]}}}}"#
        );
        fs::write(d.join(format!("{name}.json")), data).unwrap();
    }
    let s = LimitsReader::new()
        .read(&t.path().join("state"), Utc::now())
        .unwrap();
    assert_eq!(s.accounts.len(), 2);
    assert_eq!(s.accounts[0].plan.as_deref(), Some("New"));
    assert_eq!(
        s.accounts[0].checked_at,
        Utc.with_ymd_and_hms(2026, 1, 3, 0, 0, 0).unwrap()
    );
}
