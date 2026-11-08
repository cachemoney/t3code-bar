use bar_usage::{PriceBook, Record, Speed};
use serde_json::json;
fn record(model: &str) -> Record {
    Record {
        timestamp_ms: 0.0,
        model: model.into(),
        rate_model: None,
        session: String::new(),
        uncached_input: 100,
        cached_input: 20,
        cache_creation: 10,
        output: 10,
        reasoning: 0,
        reported_cost_usd: None,
        speed: Speed::Standard,
        dedupe_key: None,
        unresolved_tier_turn: None,
    }
}
#[test]
fn price_resolution_override_reported_cost_and_tiers() {
    let table = json!({"document":{
    "openai/base":{"input_cost_per_token":0.01,"output_cost_per_token":0.02,"cache_read_input_token_cost":0.002,"cache_creation_input_token_cost":0.0125,"provider_specific_entry":{"fast":2}},
    "vendor/base":{"input_cost_per_token":0.03,"output_cost_per_token":0.04},
    "vendor/priority":{"input_cost_per_token":0.01,"output_cost_per_token":0.02,"cache_read_input_token_cost":0.002,"input_cost_per_token_priority":0.03,"output_cost_per_token_priority":0.06},
    "a/conflict":{"input_cost_per_token":0.01,"output_cost_per_token":0.02},"b/conflict":{"input_cost_per_token":0.03,"output_cost_per_token":0.04}
    }});
    let settings = json!({"usagePriceOverrides":{"base":{"inputCostPerMillionTokens":1000000,"outputCostPerMillionTokens":2000000}},"usageModelAliases":{"a":"b","b":"base","loop1":"loop2","loop2":"loop1"}});
    let book = PriceBook::from_documents(&table, &settings);
    assert_eq!(book.alias("a"), Some("base"));
    assert_eq!(book.alias("loop1"), None);
    assert!(book.rate("conflict").is_none());
    assert_eq!(book.rate("BASE[1m]").unwrap().standard.input, 0.01);
    assert!(book.rate("openai/opus").is_none());
    let mut r = record("base");
    r.reported_cost_usd = Some(99.0);
    r.speed = Speed::Fast;
    let p = book.price(&r);
    assert!((p.cost_usd - 150.0).abs() < 1e-12);
    assert_eq!(p.speed_premium_usd, 0.0);
    let mut r = record("openai/base");
    r.speed = Speed::Fast;
    let list = book.price(&r);
    assert!((list.speed_premium_usd - 1.365).abs() < 1e-12);
    assert!((list.cache_savings_usd - 0.32).abs() < 1e-12);
    r.reported_cost_usd = Some(1.0);
    let priced = book.price(&r);
    let c = priced.categories.unwrap();
    assert!((c.input + c.cache_read + c.cache_write + c.output - 1.0).abs() < 1e-12);
    let mut r = record("priority");
    r.speed = Speed::Fast;
    let c = book.price(&r).categories.unwrap();
    assert!((c.cache_read - 0.12).abs() < 1e-12);
    let mut r = record("unknown");
    assert!(book.price(&r).unpriced);
    r.reported_cost_usd = Some(0.5);
    assert_eq!(book.price(&r).cost_usd, 0.5);
    assert!(!book.price(&r).unpriced);
}
