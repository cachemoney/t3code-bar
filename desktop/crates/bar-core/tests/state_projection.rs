use bar_core::{
    host::*,
    projection::*,
    threads::ThreadSummary,
    usage::{Period, PriceBook, UsageReport, Window},
};
use chrono::{TimeZone, Utc};
use std::{collections::HashMap, fs, path::PathBuf};

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn frozen_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 6, 9, 8, 0).unwrap()
}

fn build_test_usage_reports() -> HashMap<Period, UsageReport> {
    let root = fixtures_root().join("usage");
    let state = root.join("state");
    let dataset =
        bar_core::usage::decode_cache(&fs::read(state.join("usage-scan-cache-v11.json")).unwrap())
            .unwrap();
    let prices = PriceBook::load(&state);
    let now = frozen_now();
    let mut map = HashMap::new();
    for period in [Period::Day, Period::Week, Period::Month, Period::Quarter] {
        let r = UsageReport::build(
            &dataset,
            &Window::new(period, now, chrono_tz::UTC),
            &prices,
        );
        map.insert(period, r);
    }
    map
}

#[test]
fn state_projection_produces_four_state_files_with_byte_exact_dirty_flags() {
    let mut projection = StateProjection::new();
    let mut snapshot = AppSnapshot::default();
    snapshot.now = frozen_now().to_rfc3339();
    snapshot.summary = Some(ThreadSummary::default());
    snapshot.loading = false;
    snapshot.online = true;

    let view = ViewState {
        screen: "usage".into(),
        metric: "cost".into(),
        period: "24h".into(),
        generation: 1,
        visible: true,
    };
    let usage_reports = build_test_usage_reports();

    // First projection: all files are dirty
    let p1 = projection.project(&snapshot, &usage_reports, &view, chrono_tz::UTC);
    assert!(p1.dirty_state);
    assert!(p1.dirty_usage);
    assert!(p1.dirty_limits);
    assert!(p1.dirty_view);

    // Second projection with unchanged inputs: NONE are dirty
    let p2 = projection.project(&snapshot, &usage_reports, &view, chrono_tz::UTC);
    assert!(!p2.dirty_state);
    assert!(!p2.dirty_usage);
    assert!(!p2.dirty_limits);
    assert!(!p2.dirty_view);
    assert_eq!(p1.state_json, p2.state_json);
    assert_eq!(p1.usage_json, p2.usage_json);
    assert_eq!(p1.limits_json, p2.limits_json);
    assert_eq!(p1.view_json, p2.view_json);

    // Mutate view only: only view is dirty
    let mut view2 = view.clone();
    view2.metric = "tokens".into();
    let p3 = projection.project(&snapshot, &usage_reports, &view2, chrono_tz::UTC);
    assert!(!p3.dirty_state);
    assert!(!p3.dirty_usage);
    assert!(!p3.dirty_limits);
    assert!(p3.dirty_view);
}

#[test]
fn usage_json_reconciles_provider_and_model_totals_to_slots_and_period() {
    let mut projection = StateProjection::new();
    let snapshot = AppSnapshot::default();
    let view = ViewState::default();
    let usage_reports = build_test_usage_reports();

    let p = projection.project(&snapshot, &usage_reports, &view, chrono_tz::UTC);
    let val: serde_json::Value = serde_json::from_str(&p.usage_json).unwrap();
    let periods = val.get("periods").unwrap().as_object().unwrap();

    for period_key in ["24h", "7d", "30d", "90d"] {
        let period = periods.get(period_key).unwrap();
        let period_cost = period["costUsd"].as_f64().unwrap();
        let period_tokens = period["processedTokens"].as_i64().unwrap();
        let period_sessions = period["sessions"].as_u64().unwrap() as usize;

        // 1. Reconcile timeline slot providers to slot totals
        let timeline = period["timeline"].as_array().unwrap();
        for slot in timeline {
            let slot_cost = slot["costUsd"].as_f64().unwrap();
            let slot_tokens = slot["tokens"].as_i64().unwrap();
            let slot_providers = slot["providers"].as_array().unwrap();

            let sum_provider_cost: f64 = slot_providers.iter().map(|p| p["costUsd"].as_f64().unwrap()).sum();
            let sum_provider_tokens: i64 = slot_providers.iter().map(|p| p["tokens"].as_i64().unwrap()).sum();

            assert!(
                (slot_cost - sum_provider_cost).abs() < 1e-5,
                "Slot cost mismatch: {slot_cost} != {sum_provider_cost} in period {period_key}"
            );
            assert_eq!(
                slot_tokens, sum_provider_tokens,
                "Slot tokens mismatch: {slot_tokens} != {sum_provider_tokens} in period {period_key}"
            );
        }

        // 2. Reconcile period providers to period totals
        let providers = period["providers"].as_array().unwrap();
        let sum_cost: f64 = providers.iter().map(|p| p["costUsd"].as_f64().unwrap()).sum();
        let sum_tokens: i64 = providers.iter().map(|p| p["tokens"].as_i64().unwrap()).sum();
        let sum_sessions: usize = providers.iter().map(|p| p["sessions"].as_u64().unwrap() as usize).sum();

        assert!(
            (period_cost - sum_cost).abs() < 1e-4,
            "Period provider cost mismatch: {period_cost} != {sum_cost} in period {period_key}"
        );
        assert_eq!(
            period_tokens, sum_tokens,
            "Period provider tokens mismatch: {period_tokens} != {sum_tokens} in period {period_key}"
        );
        assert_eq!(
            period_sessions, sum_sessions,
            "Period provider sessions mismatch: {period_sessions} != {sum_sessions} in period {period_key}"
        );

        // 3. Reconcile period models to period totals
        let models = period["models"].as_array().unwrap();
        let model_cost: f64 = models.iter().map(|m| m["costUsd"].as_f64().unwrap()).sum();
        let model_tokens: i64 = models.iter().map(|m| m["tokens"].as_i64().unwrap()).sum();

        assert!(
            (period_cost - model_cost).abs() < 1e-4,
            "Period model cost mismatch: {period_cost} != {model_cost} in period {period_key}"
        );
        assert_eq!(
            period_tokens, model_tokens,
            "Period model tokens mismatch: {period_tokens} != {model_tokens} in period {period_key}"
        );
    }
}

#[test]
fn hostile_markup_strings_survive_projection_intact_as_plain_json_strings() {
    let mut projection = StateProjection::new();
    let mut snapshot = AppSnapshot::default();
    let hostile_title = "Title with <b>bold</b> <img onerror=alert(1)> and \u{202E}RTL";
    let hostile_branch = "feat/<b>xss</b>";
    snapshot.threads.push(ThreadItem {
        id: "hostile-1".into(),
        project_id: "p1".into(),
        project_title: "Project <script>".into(),
        title: hostile_title.into(),
        status: "Approval".into(),
        is_pinned: false,
        providers: vec!["codex".into()],
        branch: Some(hostile_branch.into()),
        model_name: Some("model <img onerror=1>".into()),
        pr_number: None,
        updated_at: "2026-11-06T09:08:00.000Z".into(),
        latest_user_message_at: None,
        latest_run_completed_at: None,
        active_run_started_at: None,
        usage_limit_reset_at: None,
        project_icon: None,
        project_favicon: None,
    });
    snapshot.summary = Some(ThreadSummary::default());

    let view = ViewState::default();
    let usage_reports = build_test_usage_reports();

    let p = projection.project(&snapshot, &usage_reports, &view, chrono_tz::UTC);

    // Verify it survives into state_json
    let val: serde_json::Value = serde_json::from_str(&p.state_json).unwrap();
    let thread = &val["threads"][0];
    assert_eq!(thread["title"].as_str().unwrap(), hostile_title);
    assert_eq!(thread["branch"].as_str().unwrap(), hostile_branch);

    // Raw JSON string must contain the exact hostile tags properly escaped as JSON
    assert!(p.state_json.contains(r#"<b>bold</b> <img onerror=alert(1)> and \u202eRTL"#)
        || p.state_json.contains(r#"<b>bold</b> <img onerror=alert(1)> and "#));
}
