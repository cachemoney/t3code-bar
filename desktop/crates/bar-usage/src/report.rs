use crate::{Dataset, Period, PriceBook, Speed, Window};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub uncached_input: i64,
    pub cached_input: i64,
    pub cache_creation: i64,
    pub output: i64,
    pub reasoning: i64,
    pub processed: i64,
}
impl Tokens {
    fn add_record(&mut self, r: &crate::Record) {
        self.uncached_input += r.uncached_input;
        self.cached_input += r.cached_input;
        self.cache_creation += r.cache_creation;
        self.output += r.output;
        self.reasoning += r.reasoning;
        self.processed += r.processed();
    }
    fn add(&mut self, r: &Self) {
        self.uncached_input += r.uncached_input;
        self.cached_input += r.cached_input;
        self.cache_creation += r.cache_creation;
        self.output += r.output;
        self.reasoning += r.reasoning;
        self.processed += r.processed;
    }
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostByType {
    pub input: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub output: f64,
    pub other: f64,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostBySpeed {
    pub standard: f64,
    pub fast: f64,
    pub ultrafast: f64,
    pub premium: f64,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub kind: String,
    pub tokens: Tokens,
    pub cost_usd: f64,
    pub sessions: usize,
    pub records: usize,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub kind: String,
    pub name: String,
    pub tokens: Tokens,
    pub cost_usd: f64,
    pub records: usize,
    pub unpriced_records: usize,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    pub cost_usd: f64,
    pub tokens: i64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub day: i64,
    pub hour_start_ms: Option<f64>,
    pub by_provider: HashMap<String, Amount>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub resolution: String,
    pub timezone: String,
    pub until_ms: f64,
    pub columns: Vec<Column>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageReport {
    pub tokens: Tokens,
    pub cost_usd: f64,
    pub cost_by_type: CostByType,
    pub cost_by_speed: CostBySpeed,
    pub cache_savings_usd: f64,
    pub providers: Vec<Provider>,
    pub models: Vec<Model>,
    pub timeline: Timeline,
    pub sessions: usize,
    pub records: usize,
    pub unpriced_records: usize,
    pub duplicates_dropped: usize,
    pub rejected_files: usize,
    pub malformed_records: i64,
    pub cache_modified: Option<chrono::DateTime<chrono::Utc>>,
    pub scan_cache_missing: bool,
    pub notices: Vec<String>,
    pub local_sources: Vec<crate::LocalStatus>,
}
#[derive(Default)]
struct Bucket {
    kind: String,
    model: String,
    tokens: Tokens,
    cost: f64,
    savings: f64,
    categories: crate::Categories,
    fast: f64,
    ultra: f64,
    premium: f64,
    records: usize,
    unpriced: usize,
}
fn rounded(v: f64) -> f64 {
    (v * 1e6 + 0.5).floor() / 1e6
}
impl UsageReport {
    pub fn build(dataset: &Dataset, window: &Window, prices: &PriceBook) -> Self {
        let hourly = window.period == Period::Day;
        let columns = if hourly {
            (0..24)
                .map(|i| {
                    let ms = window.since_ms + f64::from(i) * 3_600_000.0;
                    Column {
                        day: window.local_day(ms),
                        hour_start_ms: Some(ms),
                        by_provider: HashMap::new(),
                    }
                })
                .collect()
        } else {
            (window.since_day..=window.until_day)
                .map(|day| Column {
                    day,
                    hour_start_ms: None,
                    by_provider: HashMap::new(),
                })
                .collect()
        };
        let mut timeline = Timeline {
            resolution: if hourly { "hour" } else { "day" }.into(),
            timezone: window.timezone.to_string(),
            until_ms: if hourly { window.until_ms } else { 0.0 },
            columns,
        };
        let mut buckets: Vec<Bucket> = Vec::new();
        let mut indexes = HashMap::new();
        let selection = dataset.select(window, |kind, source, record, slot| {
            let mut record = record.clone();
            if let Some(alias) = prices.alias(&record.model) {
                record.model = alias.into();
                record.rate_model = None;
            }
            let key = (slot.day, slot.hour, source, record.model.clone());
            let index = *indexes.entry(key).or_insert_with(|| {
                buckets.push(Bucket {
                    kind: kind.into(),
                    model: record.model.clone(),
                    ..Bucket::default()
                });
                buckets.len() - 1
            });
            let priced = prices.price(&record);
            let b = &mut buckets[index];
            b.tokens.add_record(&record);
            b.cost += priced.cost_usd;
            b.savings += priced.cache_savings_usd;
            if let Some(c) = priced.categories {
                b.categories.input += c.input;
                b.categories.cache_read += c.cache_read;
                b.categories.cache_write += c.cache_write;
                b.categories.output += c.output;
            }
            match record.speed {
                Speed::Fast => b.fast += priced.cost_usd,
                Speed::Ultrafast => b.ultra += priced.cost_usd,
                _ => (),
            }
            b.premium += priced.speed_premium_usd;
            b.records += 1;
            b.unpriced += usize::from(priced.unpriced);
            let index = if hourly {
                slot.hour
            } else {
                slot.day - window.since_day
            } as usize;
            if let Some(column) = timeline.columns.get_mut(index) {
                let amount = column.by_provider.entry(kind.into()).or_default();
                amount.cost_usd += priced.cost_usd;
                amount.tokens += record.processed();
            }
        });
        let mut result = Self {
            tokens: Tokens::default(),
            cost_usd: 0.0,
            cost_by_type: CostByType::default(),
            cost_by_speed: CostBySpeed::default(),
            cache_savings_usd: 0.0,
            providers: Vec::new(),
            models: Vec::new(),
            timeline,
            sessions: selection.sessions(),
            records: 0,
            unpriced_records: 0,
            duplicates_dropped: selection.duplicates_dropped,
            rejected_files: dataset.rejected_files,
            malformed_records: selection.sources.iter().map(|s| s.malformed).sum(),
            cache_modified: None,
            scan_cache_missing: false,
            notices: vec![],
            local_sources: vec![],
        };
        let mut providers: HashMap<String, Provider> = HashMap::new();
        let mut models: HashMap<(String, String), Model> = HashMap::new();
        for t in &selection.sources {
            if t.distinct_sessions > 0 {
                let p = providers.entry(t.kind.clone()).or_insert_with(|| Provider {
                    kind: t.kind.clone(),
                    ..Provider::default()
                });
                p.sessions += t.distinct_sessions;
            }
        }
        for b in buckets {
            result.tokens.add(&b.tokens);
            result.cost_usd += b.cost;
            result.cache_savings_usd += b.savings;
            result.cost_by_type.input += rounded(b.categories.input);
            result.cost_by_type.cache_read += rounded(b.categories.cache_read);
            result.cost_by_type.cache_write += rounded(b.categories.cache_write);
            result.cost_by_type.output += rounded(b.categories.output);
            result.cost_by_speed.fast += rounded(b.fast);
            result.cost_by_speed.ultrafast += rounded(b.ultra);
            result.cost_by_speed.premium += rounded(b.premium);
            result.records += b.records;
            result.unpriced_records += b.unpriced;
            let p = providers.entry(b.kind.clone()).or_insert_with(|| Provider {
                kind: b.kind.clone(),
                ..Provider::default()
            });
            p.tokens.add(&b.tokens);
            p.cost_usd += b.cost;
            p.records += b.records;
            let m = models
                .entry((b.kind.clone(), b.model.clone()))
                .or_insert_with(|| Model {
                    kind: b.kind,
                    name: b.model,
                    ..Model::default()
                });
            m.tokens.add(&b.tokens);
            m.cost_usd += b.cost;
            m.records += b.records;
            m.unpriced_records += b.unpriced;
        }
        let c = &mut result.cost_by_type;
        c.other = (result.cost_usd - c.input - c.cache_read - c.cache_write - c.output).max(0.0);
        result.cost_by_speed.standard =
            (result.cost_usd - result.cost_by_speed.fast - result.cost_by_speed.ultrafast).max(0.0);
        let order = [
            "codex",
            "claude",
            "grok",
            "cursor",
            "opencode",
            "antigravity",
            "pi",
            "muse",
        ];
        result.providers = providers
            .into_values()
            .filter(|p| p.tokens.processed > 0 || p.cost_usd > 0.0)
            .collect();
        result.providers.sort_by(|a, b| {
            order
                .iter()
                .position(|v| *v == a.kind)
                .unwrap_or(order.len())
                .cmp(
                    &order
                        .iter()
                        .position(|v| *v == b.kind)
                        .unwrap_or(order.len()),
                )
                .then(a.kind.cmp(&b.kind))
        });
        result.models = models.into_values().collect();
        result.models.sort_by(|a, b| {
            b.cost_usd
                .total_cmp(&a.cost_usd)
                .then(b.tokens.processed.cmp(&a.tokens.processed))
                .then(a.kind.cmp(&b.kind))
                .then(a.name.cmp(&b.name))
        });
        result
    }
}
