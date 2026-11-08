use crate::{Dataset, Speed};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Duration,
};
const QUERY: &str = r#"SELECT json_extract(attempt.payload_json, '$.nativeThreadId'), json_extract(turn.payload_json, '$.nativeTurnRef.nativeId'),
 (SELECT json_extract(event.payload_json, '$.modelSelection') FROM orchestration_events AS event
  WHERE event.aggregate_kind = 'thread' AND event.application_event_version = 2 AND event.stream_id = attempt.thread_id
    AND event.event_type = 'run.updated' AND json_extract(event.payload_json, '$.id') = attempt.run_id
    AND json_extract(event.payload_json, '$.activeAttemptId') = attempt.attempt_id
    AND json_extract(event.payload_json, '$.status') = 'running' ORDER BY event.sequence LIMIT 1)
 FROM orchestration_v2_projection_provider_turns AS turn
 LEFT JOIN orchestration_v2_projection_run_attempts AS attempt
  ON attempt.attempt_id = turn.run_attempt_id AND attempt.thread_id = turn.thread_id AND attempt.provider_thread_id = turn.provider_thread_id
 WHERE json_extract(turn.payload_json, '$.nativeTurnRef.driver') = 'codex'
 AND json_extract(turn.payload_json, '$.nativeTurnRef.nativeId') IN (SELECT value FROM json_each(?))"#;
fn selection(json: &str) -> Option<(Option<String>, Speed)> {
    let value: Value = serde_json::from_str(json).ok()?;
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let options = value.get("options").and_then(Value::as_array);
    let mut tier = None;
    let mut fast = false;
    for option in options.into_iter().flatten() {
        match option.get("id").and_then(Value::as_str) {
            Some("serviceTier") => {
                let value = option.get("value")?.as_str()?;
                if tier.is_none() {
                    tier = Some(value);
                }
            }
            Some("fastMode") if option.get("value").and_then(Value::as_bool) == Some(true) => {
                fast = true
            }
            _ => (),
        }
    }
    let speed = match tier.or(if fast { Some("fast") } else { None }) {
        Some("fast" | "priority") => Speed::Fast,
        Some("ultrafast") => Speed::Ultrafast,
        None | Some("default" | "standard") => Speed::Standard,
        _ => return None,
    };
    Some((model, speed))
}
/// Resolve pending turns using a read-only T3 Code database. Errors leave records unchanged.
pub fn resolve_codex_tiers(dataset: &mut Dataset, path: &Path) {
    let turns: HashSet<_> = dataset
        .files
        .iter()
        .filter(|f| f.kind == "codex")
        .flat_map(|f| {
            f.records
                .iter()
                .filter_map(|r| r.unresolved_tier_turn.as_deref())
        })
        .collect();
    if turns.is_empty() {
        return;
    }
    let Ok(connection) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    ) else {
        return;
    };
    if connection.busy_timeout(Duration::from_millis(250)).is_err() {
        return;
    }
    let Ok(mut statement) = connection.prepare(QUERY) else {
        return;
    };
    let Ok(json) = serde_json::to_string(&turns) else {
        return;
    };
    let Ok(rows) = statement.query_map([json], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    }) else {
        return;
    };
    let mut resolved = HashMap::new();
    for row in rows {
        let Ok((thread, turn, model)) = row else {
            return;
        };
        if let Some(turn) = turn {
            use std::collections::hash_map::Entry;
            match resolved.entry(turn) {
                Entry::Vacant(e) => {
                    e.insert(Some((thread, model)));
                }
                Entry::Occupied(mut e) => {
                    e.insert(None);
                }
            }
        }
    }
    for file in &mut dataset.files {
        if file.kind != "codex" {
            continue;
        }
        for record in &mut file.records {
            let Some((thread, Some(json))) = record
                .unresolved_tier_turn
                .as_ref()
                .and_then(|turn| resolved.get(turn))
                .and_then(Option::as_ref)
            else {
                continue;
            };
            let Some((model, speed)) = selection(json) else {
                continue;
            };
            if thread.as_deref() == Some(record.session.as_str())
                && model.as_deref() == Some(record.model.as_str())
            {
                record.speed = speed;
                record.unresolved_tier_turn = None;
            }
        }
    }
}
