use bar_usage::{decode_cache, resolve_codex_tiers, Speed};
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};
#[test]
fn tier_requires_unique_turn_and_matching_session_and_model() {
    let path = std::env::temp_dir().join(format!(
        "bar-usage-tier-{}-{}.sqlite",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let db = Connection::open(&path).unwrap();
    db.execute_batch(r#"
      CREATE TABLE orchestration_v2_projection_provider_turns (payload_json TEXT, run_attempt_id TEXT, thread_id TEXT, provider_thread_id TEXT);
      CREATE TABLE orchestration_v2_projection_run_attempts (payload_json TEXT, attempt_id TEXT, thread_id TEXT, provider_thread_id TEXT, run_id TEXT);
      CREATE TABLE orchestration_events (aggregate_kind TEXT, application_event_version INTEGER, stream_id TEXT, event_type TEXT, payload_json TEXT, sequence INTEGER);
      INSERT INTO orchestration_v2_projection_provider_turns VALUES ('{"nativeTurnRef":{"driver":"codex","nativeId":"turn"}}','attempt','thread','provider');
      INSERT INTO orchestration_v2_projection_run_attempts VALUES ('{"nativeThreadId":"session"}','attempt','thread','provider','run');
      INSERT INTO orchestration_events VALUES ('thread',2,'thread','run.updated','{"id":"run","activeAttemptId":"attempt","status":"running","modelSelection":{"model":"model","options":[{"id":"serviceTier","value":"priority"}]}}',1);
    "#).unwrap();
    let json = br#"{"version":11,"models":["model","different"],"sessions":["session","wrong"],"files":{"/a":{"p":"codex","m":1,"r":[[1,0,0,1,0,0,0,0,null,null,0,null,"turn"],[1,1,0,1,0,0,0,0,null,null,0,null,"turn"],[1,0,1,1,0,0,0,0,null,null,0,null,"turn"]]}}}"#;
    let mut data = decode_cache(json).unwrap();
    resolve_codex_tiers(&mut data, &path);
    assert_eq!(
        data.files[0]
            .records
            .iter()
            .map(|r| r.speed)
            .collect::<Vec<_>>(),
        [Speed::Fast, Speed::Standard, Speed::Standard]
    );
    db.execute("INSERT INTO orchestration_v2_projection_provider_turns SELECT * FROM orchestration_v2_projection_provider_turns", []).unwrap();
    let mut ambiguous = decode_cache(json).unwrap();
    resolve_codex_tiers(&mut ambiguous, &path);
    assert_eq!(ambiguous.files[0].records[0].speed, Speed::Standard);
    drop(db);
    std::fs::remove_file(path).unwrap();
}
