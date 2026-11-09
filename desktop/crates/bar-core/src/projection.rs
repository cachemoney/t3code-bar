use crate::{
    host::{provider_label, AppSnapshot, ThreadItem, ViewState},
    settings::Settings,
    threads::ThreadSummary,
    usage::{self, Period, UsageReport},
};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StateFilePayload {
    pub now: String,
    pub loading: bool,
    pub online: bool,
    pub startup_error: Option<String>,
    pub thread_error: Option<String>,
    pub version: String,
    pub summary: ThreadSummary,
    pub connection: crate::host::ConnectionState,
    pub settings: Settings,
    pub threads: Vec<ThreadItem>,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LimitsFilePayload {
    pub groups: Vec<serde_json::Value>,
    pub notices: Vec<String>,
    pub links: Vec<serde_json::Value>,
    pub checked_at: Option<String>,
    pub read_failed: bool,
    pub skipped_files: usize,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ViewFilePayload {
    pub visible: bool,
    pub screen: String,
    pub metric: String,
    pub period: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageSlotProviderPayload {
    pub kind: String,
    pub cost_usd: f64,
    pub tokens: i64,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageTimelineSlotPayload {
    pub label: String,
    pub cost_usd: f64,
    pub tokens: i64,
    pub providers: Vec<UsageSlotProviderPayload>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageProviderPayload {
    pub kind: String,
    pub label: String,
    pub cost_usd: f64,
    pub tokens: i64,
    pub sessions: usize,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageModelPayload {
    pub kind: String,
    pub name: String,
    pub cost_usd: f64,
    pub tokens: i64,
    pub records: usize,
    pub unpriced_records: usize,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageTokensPayload {
    pub uncached_input: i64,
    pub cached_input: i64,
    pub cache_creation: i64,
    pub output: i64,
    pub processed: i64,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageCostByTypePayload {
    pub input: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub output: f64,
    pub other: f64,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageCostBySpeedPayload {
    pub standard: f64,
    pub fast: f64,
    pub ultrafast: f64,
    pub premium: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsagePeriodPayload {
    pub cost_usd: f64,
    pub processed_tokens: i64,
    pub sessions: usize,
    pub timeline: Vec<UsageTimelineSlotPayload>,
    pub providers: Vec<UsageProviderPayload>,
    pub models: Vec<UsageModelPayload>,
    pub tokens_breakdown: UsageTokensPayload,
    pub cost_by_type: UsageCostByTypePayload,
    pub cost_by_speed: UsageCostBySpeedPayload,
    pub cache_savings_usd: f64,
    pub records: usize,
    pub unpriced_records: usize,
    pub malformed_records: i64,
    pub rejected_files: usize,
    pub cache_modified: Option<String>,
    pub scan_cache_missing: bool,
    pub local_sources_partial: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct UsagePeriods {
    #[serde(rename = "24h")]
    pub day: UsagePeriodPayload,
    #[serde(rename = "7d")]
    pub week: UsagePeriodPayload,
    #[serde(rename = "30d")]
    pub month: UsagePeriodPayload,
    #[serde(rename = "90d")]
    pub quarter: UsagePeriodPayload,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageFilePayload {
    pub periods: UsagePeriods,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedStateFiles {
    pub state_json: String,
    pub dirty_state: bool,
    pub usage_json: String,
    pub dirty_usage: bool,
    pub limits_json: String,
    pub dirty_limits: bool,
    pub view_json: String,
    pub dirty_view: bool,
}

#[derive(Default)]
pub struct StateProjection {
    last_state_json: Option<String>,
    last_usage_json: Option<String>,
    last_limits_json: Option<String>,
    last_view_json: Option<String>,
}

impl StateProjection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn project(
        &mut self,
        snapshot: &AppSnapshot,
        usage_reports: &HashMap<Period, UsageReport>,
        view: &ViewState,
        timezone: Tz,
    ) -> ProjectedStateFiles {
        // 1. state.json payload
        let state_payload = StateFilePayload {
            now: snapshot.now.clone(),
            loading: snapshot.loading,
            online: snapshot.online,
            startup_error: snapshot.startup_error.clone(),
            thread_error: snapshot.thread_error.clone(),
            version: snapshot.version.clone(),
            summary: snapshot.summary.unwrap_or_default(),
            connection: snapshot.connection.clone(),
            settings: snapshot.settings.clone(),
            threads: snapshot.threads.clone(),
        };
        let state_json = serde_json::to_string_pretty(&state_payload).unwrap() + "\n";
        let dirty_state = self.last_state_json.as_deref() != Some(&state_json);
        self.last_state_json = Some(state_json.clone());

        // 2. limits.json payload
        let limits_payload = if let Some(l) = &snapshot.limits {
            LimitsFilePayload {
                groups: l.groups.clone(),
                notices: l.notices.clone(),
                links: l.links.clone(),
                checked_at: l.checked_at.clone(),
                read_failed: l.read_failed,
                skipped_files: l.skipped_files,
            }
        } else {
            LimitsFilePayload::default()
        };
        let limits_json = serde_json::to_string_pretty(&limits_payload).unwrap() + "\n";
        let dirty_limits = self.last_limits_json.as_deref() != Some(&limits_json);
        self.last_limits_json = Some(limits_json.clone());

        // 3. view.json payload
        let view_payload = ViewFilePayload {
            visible: view.visible,
            screen: view.screen.clone(),
            metric: view.metric.clone(),
            period: view.period.clone(),
        };
        let view_json = serde_json::to_string_pretty(&view_payload).unwrap() + "\n";
        let dirty_view = self.last_view_json.as_deref() != Some(&view_json);
        self.last_view_json = Some(view_json.clone());

        // 4. usage.json payload
        let empty_report = UsageReport {
            tokens: usage::Tokens::default(),
            cost_usd: 0.0,
            cost_by_type: usage::CostByType::default(),
            cost_by_speed: usage::CostBySpeed::default(),
            cache_savings_usd: 0.0,
            providers: vec![],
            models: vec![],
            timeline: usage::Timeline {
                resolution: "day".into(),
                timezone: timezone.to_string(),
                until_ms: 0.0,
                columns: vec![],
            },
            sessions: 0,
            records: 0,
            unpriced_records: 0,
            duplicates_dropped: 0,
            rejected_files: 0,
            malformed_records: 0,
            cache_modified: None,
            scan_cache_missing: true,
            notices: vec![],
            local_sources: vec![],
        };

        let day_report = usage_reports.get(&Period::Day).unwrap_or(&empty_report);
        let week_report = usage_reports.get(&Period::Week).unwrap_or(&empty_report);
        let month_report = usage_reports.get(&Period::Month).unwrap_or(&empty_report);
        let quarter_report = usage_reports.get(&Period::Quarter).unwrap_or(&empty_report);

        let usage_payload = UsageFilePayload {
            periods: UsagePeriods {
                day: project_period(day_report, Period::Day, timezone),
                week: project_period(week_report, Period::Week, timezone),
                month: project_period(month_report, Period::Month, timezone),
                quarter: project_period(quarter_report, Period::Quarter, timezone),
            },
        };
        let usage_json = serde_json::to_string_pretty(&usage_payload).unwrap() + "\n";
        let dirty_usage = self.last_usage_json.as_deref() != Some(&usage_json);
        self.last_usage_json = Some(usage_json.clone());

        ProjectedStateFiles {
            state_json,
            dirty_state,
            usage_json,
            dirty_usage,
            limits_json,
            dirty_limits,
            view_json,
            dirty_view,
        }
    }
}

fn project_period(r: &UsageReport, period: Period, timezone: Tz) -> UsagePeriodPayload {
    let timeline: Vec<UsageTimelineSlotPayload> = r
        .timeline
        .columns
        .iter()
        .enumerate()
        .map(|(idx, col)| {
            let label = match period {
                Period::Day => {
                    if let Some(ms) = col.hour_start_ms {
                        let dt = DateTime::from_timestamp_millis(ms as i64)
                            .unwrap_or_else(|| Utc::now());
                        let s = dt.with_timezone(&timezone).format("%-I%P").to_string();
                        s.trim_end_matches('m').to_string()
                    } else {
                        format!("H{idx}")
                    }
                }
                Period::Week => {
                    let dt = DateTime::from_timestamp(col.day * 86400, 0)
                        .unwrap_or_else(|| Utc::now());
                    dt.with_timezone(&timezone).format("%a").to_string()
                }
                Period::Month => format!("D{}", idx + 1),
                Period::Quarter => format!("W{}", idx + 1),
            };

            let mut slot_providers: Vec<UsageSlotProviderPayload> = col
                .by_provider
                .iter()
                .map(|(k, v)| UsageSlotProviderPayload {
                    kind: k.clone(),
                    cost_usd: v.cost_usd,
                    tokens: v.tokens,
                })
                .collect();
            slot_providers.sort_by(|a, b| a.kind.cmp(&b.kind));

            let slot_cost: f64 = slot_providers.iter().map(|p| p.cost_usd).sum();
            let slot_tokens: i64 = slot_providers.iter().map(|p| p.tokens).sum();

            UsageTimelineSlotPayload {
                label,
                cost_usd: slot_cost,
                tokens: slot_tokens,
                providers: slot_providers,
            }
        })
        .collect();

    let providers: Vec<UsageProviderPayload> = r
        .providers
        .iter()
        .map(|p| UsageProviderPayload {
            kind: p.kind.clone(),
            label: provider_label(&p.kind),
            cost_usd: p.cost_usd,
            tokens: p.tokens.processed,
            sessions: p.sessions,
        })
        .collect();

    let models: Vec<UsageModelPayload> = r
        .models
        .iter()
        .map(|m| UsageModelPayload {
            kind: m.kind.clone(),
            name: m.name.clone(),
            cost_usd: m.cost_usd,
            tokens: m.tokens.processed,
            records: m.records,
            unpriced_records: m.unpriced_records,
        })
        .collect();

    UsagePeriodPayload {
        cost_usd: r.cost_usd,
        processed_tokens: r.tokens.processed,
        sessions: r.sessions,
        timeline,
        providers,
        models,
        tokens_breakdown: UsageTokensPayload {
            uncached_input: r.tokens.uncached_input,
            cached_input: r.tokens.cached_input,
            cache_creation: r.tokens.cache_creation,
            output: r.tokens.output,
            processed: r.tokens.processed,
        },
        cost_by_type: UsageCostByTypePayload {
            input: r.cost_by_type.input,
            cache_read: r.cost_by_type.cache_read,
            cache_write: r.cost_by_type.cache_write,
            output: r.cost_by_type.output,
            other: r.cost_by_type.other,
        },
        cost_by_speed: UsageCostBySpeedPayload {
            standard: r.cost_by_speed.standard,
            fast: r.cost_by_speed.fast,
            ultrafast: r.cost_by_speed.ultrafast,
            premium: r.cost_by_speed.premium,
        },
        cache_savings_usd: r.cache_savings_usd,
        records: r.records,
        unpriced_records: r.unpriced_records,
        malformed_records: r.malformed_records,
        rejected_files: r.rejected_files,
        cache_modified: r.cache_modified.map(|dt| dt.to_rfc3339()),
        scan_cache_missing: r.scan_cache_missing,
        local_sources_partial: r.local_sources.iter().any(|s| s.failed_files > 0),
    }
}
