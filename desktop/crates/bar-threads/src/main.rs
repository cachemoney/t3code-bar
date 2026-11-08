use bar_threads::{export_json, read_preview, read_threads, Configuration, ThreadSummary};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || args.len() > 5 {
        return Err(
            "usage: bar-threads STATE_DIR RFC3339_NOW [PREVIEW_ID] [online|offline]".into(),
        );
    }
    let c = Configuration::from_environment(Some(PathBuf::from(&args[1])));
    let now = DateTime::parse_from_rfc3339(&args[2])?.with_timezone(&Utc);
    let mut snapshot = read_threads(&c, now)?;
    if let Some(mode) = args.get(4) {
        let online = match mode.as_str() {
            "online" => true,
            "offline" => false,
            _ => return Err("expected online or offline".into()),
        };
        snapshot.is_server_online = online;
        snapshot.summary = ThreadSummary::new(&snapshot.threads, online);
    }
    let preview = match args.get(3) {
        Some(id) => read_preview(&c, id)?,
        None => None,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&export_json(&snapshot, preview.as_ref()))?
    );
    Ok(())
}
