use bar_usage::{decode_cache, Period, PriceBook, UsageReport, Window};
use chrono::{TimeZone, Utc};
use serde_json::Value;
fn num(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap()
}
fn close(a: f64, b: f64, label: &str) {
    assert!((a - b).abs() < 1e-9, "{label}: {a} != {b}");
}
#[test]
fn report_matches_canary_for_all_periods() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/usage");
    let state = root.join("state");
    let dataset =
        decode_cache(&std::fs::read(state.join("usage-scan-cache-v11.json")).unwrap()).unwrap();
    let golden: Value =
        serde_json::from_slice(&std::fs::read(root.join("golden.json")).unwrap()).unwrap();
    let prices = PriceBook::load(&state);
    let now = Utc.with_ymd_and_hms(2026, 11, 6, 9, 8, 0).unwrap();
    for (period, label) in [
        (Period::Day, "Past 24h"),
        (Period::Week, "7 days"),
        (Period::Month, "30 days"),
        (Period::Quarter, "90 days"),
    ] {
        let r = UsageReport::build(
            &dataset,
            &Window::new(period, now, chrono_tz::Asia::Ho_Chi_Minh),
            &prices,
        );
        let g = &golden["periods"][label];
        close(r.cost_usd, num(g, "costUsd"), label);
        close(r.cache_savings_usd, num(g, "cacheSavingsUsd"), label);
        assert_eq!(r.unpriced_records as f64, num(g, "unpricedRecords"));
        for (a, k) in [
            (r.cost_by_type.input, "input"),
            (r.cost_by_type.cache_read, "cacheRead"),
            (r.cost_by_type.cache_write, "cacheWrite"),
            (r.cost_by_type.output, "output"),
            (r.cost_by_type.other, "other"),
        ] {
            close(a, num(&g["costByType"], k), label);
        }
        for (a, k) in [
            (r.cost_by_speed.standard, "standard"),
            (r.cost_by_speed.fast, "fast"),
            (r.cost_by_speed.ultrafast, "ultrafast"),
            (r.cost_by_speed.premium, "premium"),
        ] {
            close(a, num(&g["costBySpeed"], k), label);
        }
        assert_eq!(r.providers.len(), g["providers"].as_object().unwrap().len());
        for p in &r.providers {
            let v = &g["providers"][&p.kind];
            assert_eq!(p.tokens.processed as f64, num(v, "tokens"));
            assert_eq!(p.sessions as f64, num(v, "sessions"));
            close(p.cost_usd, num(v, "cost"), label);
        }
        assert_eq!(r.models.len(), g["models"].as_object().unwrap().len());
        for m in &r.models {
            let v = &g["models"][format!("{}/{}", m.kind, m.name)];
            assert_eq!(m.tokens.processed as f64, num(v, "tokens"));
            assert_eq!(m.unpriced_records as f64, num(v, "unpriced"));
            close(m.cost_usd, num(v, "cost"), label);
        }
        let expected = g["series"]["columns"].as_object().unwrap();
        for (i, c) in r.timeline.columns.iter().enumerate() {
            let key = if period == Period::Day {
                i.to_string()
            } else {
                chrono::DateTime::from_timestamp(c.day * 86400, 0)
                    .unwrap()
                    .format("%Y-%m-%d")
                    .to_string()
            };
            let empty = serde_json::Map::new();
            let e = expected
                .get(&key)
                .and_then(Value::as_object)
                .unwrap_or(&empty);
            let seen: Vec<_> = c
                .by_provider
                .iter()
                .filter(|(_, v)| v.tokens > 0 || v.cost_usd > 0.0)
                .collect();
            assert_eq!(seen.len(), e.len(), "{label} {key}");
            for (kind, a) in seen {
                let v = &e[kind];
                assert_eq!(a.tokens as f64, num(v, "tokens"));
                close(a.cost_usd, num(v, "cost"), label);
            }
        }
    }
}
