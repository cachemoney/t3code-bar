use bar_usage::{Period, UsageReader, UsageReport};
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};
use std::{env, path::PathBuf};
fn period_value(r: UsageReport, hourly: bool) -> Value {
    let providers: Map<String, Value> = r.providers.iter().map(|p| (p.kind.clone(), json!({"cost":p.cost_usd,"records":p.records,"sessions":p.sessions,"tokens":p.tokens.processed}))).collect();
    let models: Map<String, Value> = r.models.iter().map(|m| (format!("{}/{}",m.kind,m.name), json!({"cost":m.cost_usd,"records":m.records,"tokens":m.tokens.processed,"unpriced":m.unpriced_records}))).collect();
    let columns: Vec<Value> = r
        .timeline
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let slot = if hourly {
                i.to_string()
            } else {
                DateTime::from_timestamp(c.day * 86400, 0)
                    .unwrap()
                    .format("%Y-%m-%d")
                    .to_string()
            };
            let providers: Map<String, Value> = c
                .by_provider
                .iter()
                .map(|(k, v)| (k.clone(), json!({"costUsd":v.cost_usd,"tokens":v.tokens})))
                .collect();
            let cost: f64 = c.by_provider.values().map(|v| v.cost_usd).sum();
            let tokens: i64 = c.by_provider.values().map(|v| v.tokens).sum();
            json!({"slot":slot,"costUsd":cost,"tokens":tokens,"providers":providers})
        })
        .collect();
    json!({"totalTokens":r.tokens.processed,"uncachedInput":r.tokens.uncached_input,"cachedInput":r.tokens.cached_input,"cacheCreation":r.tokens.cache_creation,"output":r.tokens.output,"reasoning":r.tokens.reasoning,"costUsd":r.cost_usd,"cacheSavingsUsd":r.cache_savings_usd,"costByType":r.cost_by_type,"costBySpeed":r.cost_by_speed,"sessions":r.sessions,"records":r.records,"unpricedRecords":r.unpriced_records,"duplicatesDropped":r.duplicates_dropped,"providerOrder":r.providers.iter().map(|p| &p.kind).collect::<Vec<_>>(),"providers":providers,"models":models,"timeline":columns})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let discover = args.iter().any(|arg| arg == "--discover");
    let positional: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|arg| arg.as_str() != "--discover")
        .collect();
    if positional.len() != 4 || positional[0] != "--export" {
        return Err("usage: export --export STATE_DIR ISO_CLOCK TIME_ZONE [--discover]".into());
    }
    let now: DateTime<Utc> = DateTime::parse_from_rfc3339(positional[2])?.into();
    let zone = positional[3].parse()?;
    let state_dir = PathBuf::from(positional[1]);
    let mut reader = UsageReader::new(state_dir.clone());
    if discover {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME unavailable")?;
        reader.set_local_roots(bar_usage::LocalRoots::discover(&state_dir, home));
    }
    let mut periods = Map::new();
    for (period, label) in [
        (Period::Day, "Past 24h"),
        (Period::Week, "7 days"),
        (Period::Month, "30 days"),
        (Period::Quarter, "90 days"),
    ] {
        periods.insert(label.into(), {
            let report = reader.read(period, now, zone)?;
            let mut value = period_value(report.clone(), period == Period::Day);
            if discover {
                value["localSources"] = serde_json::to_value(report.local_sources)?;
            }
            value
        });
    }
    let mut export =
        json!({"schemaVersion":1,"clock":positional[2],"timeZone":positional[3],"periods":periods});
    if discover {
        export["discoverLocalSources"] = json!(true);
    }
    println!("{}", serde_json::to_string_pretty(&export)?);
    Ok(())
}
