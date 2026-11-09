use crate::{
    attention::{AttentionAlert, AttentionTracker},
    limits,
    settings::{SettingKey, Settings, SettingsStore},
    threads::{self, ThreadSummary},
    usage,
};
use base64::Engine;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotMode {
    Tauri,
    HostBridge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FaviconMode {
    DataUrl,
    CachePath,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadItem {
    pub id: String,
    pub project_id: String,
    pub project_title: String,
    pub title: String,
    pub status: String,
    pub is_pinned: bool,
    pub providers: Vec<String>,
    pub branch: Option<String>,
    pub model_name: Option<String>,
    pub pr_number: Option<i64>,
    pub updated_at: String,
    pub latest_user_message_at: Option<String>,
    pub latest_run_completed_at: Option<String>,
    pub active_run_started_at: Option<String>,
    pub usage_limit_reset_at: Option<String>,
    pub project_icon: Option<String>,
    pub project_favicon: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub uncached_input: i64,
    pub cached_input: i64,
    pub cache_creation: i64,
    pub output: i64,
    pub reasoning: i64,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageReport {
    pub cost_usd: f64,
    pub processed_tokens: i64,
    pub tokens: Tokens,
    pub sessions: usize,
    pub records: usize,
    pub unpriced_records: usize,
    pub duplicates_dropped: usize,
    pub cache_savings_usd: f64,
    pub malformed_records: i64,
    pub rejected_files: usize,
    pub cache_modified: Option<String>,
    pub scan_cache_missing: bool,
    pub local_sources_partial: bool,
    pub cost_by_type: serde_json::Value,
    pub cost_by_speed: serde_json::Value,
    pub providers: Vec<serde_json::Value>,
    pub models: Vec<serde_json::Value>,
    pub timeline: Vec<serde_json::Value>,
    pub resolution: String,
    pub time_zone: String,
    pub until_ms: f64,
    pub notices: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LimitsReport {
    pub groups: Vec<serde_json::Value>,
    pub notices: Vec<String>,
    pub checked_at: Option<String>,
    pub links: Vec<serde_json::Value>,
    pub read_failed: bool,
    pub skipped_files: usize,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionState {
    pub status: String,
    pub message: String,
    pub connected: bool,
    pub paired: bool,
    pub has_data: bool,
    pub port: i64,
    pub updated_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateState {
    pub status: String,
    pub message: String,
    pub version: Option<String>,
    pub offer_id: Option<u64>,
    pub can_install: bool,
    pub busy: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub startup_error: Option<String>,
    pub now: String,
    pub threads: Vec<ThreadItem>,
    pub online: bool,
    pub thread_error: Option<String>,
    pub loading: bool,
    pub connection: ConnectionState,
    pub usage: Option<UsageReport>,
    pub usage_error: Option<String>,
    pub usage_loading: bool,
    pub usage_period: String,
    pub limits: Option<LimitsReport>,
    pub settings: Settings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<UpdateState>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ThreadSummary>,
}

impl Default for AppSnapshot {
    fn default() -> Self {
        Self {
            startup_error: None,
            now: Utc::now().to_rfc3339(),
            threads: vec![],
            online: false,
            thread_error: None,
            loading: true,
            connection: ConnectionState {
                status: "unavailable".into(),
                message: "T3 Code is unavailable. Showing saved data when present.".into(),
                connected: false,
                paired: false,
                has_data: false,
                port: 0,
                updated_at: None,
            },
            usage: None,
            usage_error: None,
            usage_loading: false,
            usage_period: "24h".into(),
            limits: None,
            settings: Settings::default(),
            update: Some(UpdateState {
                status: "unavailable".into(),
                message: "Updates are unavailable in this build.".into(),
                version: None,
                offer_id: None,
                can_install: false,
                busy: false,
            }),
            version: env!("CARGO_PKG_VERSION").into(),
            summary: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewState {
    pub screen: String,
    pub metric: String,
    pub period: String,
    #[serde(default)]
    pub generation: u64,
    #[serde(default)]
    pub visible: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            screen: "threads".into(),
            metric: "cost".into(),
            period: "24h".into(),
            generation: 0,
            visible: false,
        }
    }
}

pub trait HostEventSink: Send + Sync + 'static {
    fn on_snapshot_changed(&self, _snapshot: &AppSnapshot) {}
    fn on_tray_summary(&self, _summary: &ThreadSummary) {}
    fn on_attention_alerts(&self, _alerts: &[AttentionAlert]) {}
    fn on_settings_changed(&self, _settings: &Settings) {}
}

#[derive(Default)]
pub struct NoopEventSink;
impl HostEventSink for NoopEventSink {}

type FaviconKey = (Option<String>, Option<String>, FaviconMode);
type FaviconCache = HashMap<FaviconKey, Option<String>>;

pub struct Favicons {
    cache: Mutex<FaviconCache>,
    cache_dir: PathBuf,
}

impl Default for Favicons {
    fn default() -> Self {
        let dir = std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs_fallback().join("state")
            })
            .join("t3code-bar/favicons");
        Self::new(dir)
    }
}

fn dirs_fallback() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".local"))
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

impl Favicons {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            cache_dir,
        }
    }

    pub fn resolve(&self, root: Option<&str>, explicit: Option<&str>) -> Option<String> {
        self.resolve_mode(root, explicit, FaviconMode::DataUrl)
    }

    pub fn resolve_mode(
        &self,
        root: Option<&str>,
        explicit: Option<&str>,
        mode: FaviconMode,
    ) -> Option<String> {
        let key = (root.map(str::to_owned), explicit.map(str::to_owned), mode);
        let mut cache = self.cache.lock().unwrap();
        cache
            .entry(key)
            .or_insert_with(|| {
                let root = root.filter(|s| !s.is_empty()).map(Path::new);
                let mut candidates = Vec::<PathBuf>::new();
                if let Some(path) = explicit.filter(|s| !s.is_empty()).map(Path::new) {
                    if path.is_absolute() {
                        candidates.push(path.into());
                    } else if let Some(root) = root {
                        candidates.push(root.join(path));
                    }
                }
                if let Some(root) = root {
                    for path in [
                        "favicon.svg",
                        "favicon.ico",
                        "favicon.png",
                        "public/favicon.svg",
                        "public/favicon.ico",
                        "public/favicon.png",
                        "app/favicon.ico",
                        "app/favicon.png",
                        "app/icon.svg",
                        "app/icon.png",
                        "app/icon.ico",
                        "src/favicon.ico",
                        "src/favicon.svg",
                        "src/app/favicon.ico",
                        "src/app/icon.svg",
                        "src/app/icon.png",
                        "assets/icon.svg",
                        "assets/icon.png",
                        "assets/logo.svg",
                        "assets/logo.png",
                        ".idea/icon.svg",
                        "Resources/AppIcon.icns",
                        "AppIcon.icns",
                    ] {
                        candidates.push(root.join(path));
                    }
                }
                candidates
                    .iter()
                    .find_map(|path| self.render_favicon(path, mode))
            })
            .clone()
    }

    fn render_favicon(&self, path: &Path, mode: FaviconMode) -> Option<String> {
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        if bytes.len() > 2 * 1024 * 1024 {
            return None;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        let mime = match ext.as_str() {
            "svg" => "image/svg+xml",
            "png" => "image/png",
            "ico" => "image/x-icon",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => return None,
        };

        match mode {
            FaviconMode::DataUrl => Some(format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            )),
            FaviconMode::CachePath => {
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let hash = format!("{:x}", hasher.finalize());
                let out_name = format!("{hash}.{ext}");
                let out_path = self.cache_dir.join(&out_name);
                if !out_path.exists() {
                    let _ = std::fs::create_dir_all(&self.cache_dir);
                    let _ = std::fs::write(&out_path, &bytes);
                }
                Some(out_path.to_str()?.to_owned())
            }
        }
    }
}

pub fn iso(d: Option<DateTime<Utc>>) -> Option<String> {
    d.map(|v| v.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

pub fn provider_label(kind: &str) -> String {
    match kind {
        "codex" => "Codex".into(),
        "claudeAgent" | "claude" => "Claude".into(),
        "cursor" => "Cursor".into(),
        "grok" => "Grok".into(),
        "opencode" => "OpenCode".into(),
        "antigravity" => "Antigravity".into(),
        other => other.to_owned(),
    }
}

pub fn thread_item(
    t: threads::ThreadItem,
    favicons: &Favicons,
    mode: SnapshotMode,
    fixture: bool,
) -> ThreadItem {
    let project_favicon = if fixture {
        None
    } else {
        let fmode = match mode {
            SnapshotMode::Tauri => FaviconMode::DataUrl,
            SnapshotMode::HostBridge => FaviconMode::CachePath,
        };
        favicons.resolve_mode(
            t.workspace_root.as_deref(),
            t.project_favicon_path.as_deref(),
            fmode,
        )
    };
    ThreadItem {
        project_favicon,
        id: t.id,
        project_id: t.project_id,
        project_title: t.project_title,
        title: t.title,
        status: format!("{:?}", t.status),
        is_pinned: t.is_pinned,
        providers: t.providers,
        branch: t.branch,
        model_name: t.model_name,
        pr_number: t.pr_number,
        updated_at: t
            .updated_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        latest_user_message_at: iso(t.latest_user_message_at),
        latest_run_completed_at: iso(t.latest_run_completed_at),
        active_run_started_at: iso(t.active_run_started_at),
        usage_limit_reset_at: iso(t.usage_limit_reset_at),
        project_icon: t.project_icon_json,
    }
}

pub fn report_usage(r: usage::UsageReport, timezone: Tz) -> UsageReport {
    let providers = r
        .providers
        .iter()
        .map(|p| {
            json!({
                "kind": p.kind,
                "label": provider_label(&p.kind),
                "costUsd": p.cost_usd,
                "processedTokens": p.tokens.processed,
                "sessions": p.sessions,
                "records": p.records
            })
        })
        .collect();
    let models = r
        .models
        .iter()
        .map(|m| {
            json!({
                "kind": m.kind,
                "name": m.name,
                "costUsd": m.cost_usd,
                "processedTokens": m.tokens.processed,
                "records": m.records,
                "unpricedRecords": m.unpriced_records
            })
        })
        .collect();
    let timeline = r
        .timeline
        .columns
        .iter()
        .map(|c| {
            let mut costs: Vec<_> = c
                .by_provider
                .iter()
                .map(|(k, v)| {
                    json!({
                        "kind": k,
                        "costUsd": v.cost_usd,
                        "processedTokens": v.tokens
                    })
                })
                .collect();
            costs.sort_by(|a, b| {
                a["kind"].as_str().unwrap_or("").cmp(b["kind"].as_str().unwrap_or(""))
            });
            json!({
                "day": c.day,
                "hourStartMs": c.hour_start_ms,
                "label": if let Some(ms) = c.hour_start_ms {
                    DateTime::from_timestamp_millis(ms as i64)
                        .map(|d| d.with_timezone(&timezone).format("%-I %p").to_string())
                        .unwrap_or_default()
                } else {
                    DateTime::from_timestamp(c.day * 86400, 0)
                        .ok_or(())
                        .map(|d| d.format("%b %-d").to_string())
                        .unwrap_or_default()
                },
                "costUsd": costs.iter().filter_map(|v| v.get("costUsd").and_then(|x| x.as_f64())).sum::<f64>(),
                "processedTokens": costs.iter().filter_map(|v| v.get("processedTokens").and_then(|x| x.as_i64())).sum::<i64>(),
                "providers": costs
            })
        })
        .collect();
    let tokens = Tokens {
        uncached_input: r.tokens.uncached_input,
        cached_input: r.tokens.cached_input,
        cache_creation: r.tokens.cache_creation,
        output: r.tokens.output,
        reasoning: r.tokens.reasoning,
    };
    UsageReport {
        cost_usd: r.cost_usd,
        processed_tokens: r.tokens.processed,
        tokens,
        sessions: r.sessions,
        records: r.records,
        unpriced_records: r.unpriced_records,
        duplicates_dropped: r.duplicates_dropped,
        cache_savings_usd: r.cache_savings_usd,
        malformed_records: r.malformed_records,
        rejected_files: r.rejected_files,
        cache_modified: iso(r.cache_modified),
        scan_cache_missing: r.scan_cache_missing,
        local_sources_partial: r
            .local_sources
            .iter()
            .any(|source| source.state == "partial" || source.state == "unreadable"),
        cost_by_type: serde_json::to_value(r.cost_by_type).unwrap_or_default(),
        cost_by_speed: serde_json::to_value(r.cost_by_speed).unwrap_or_default(),
        providers,
        models,
        timeline,
        resolution: r.timeline.resolution,
        time_zone: r.timeline.timezone,
        until_ms: r.timeline.until_ms,
        notices: r.notices,
    }
}

pub fn report_limits(
    s: limits::LimitsSnapshot,
    now: DateTime<Utc>,
    read_failed: bool,
) -> LimitsReport {
    let groups = s
        .groups(now)
        .into_iter()
        .map(|g| {
            let rows = g
                .rows
                .into_iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "label": r.label,
                        "remainingPercent": r.remaining_percent,
                        "usedPercent": r.used_percent,
                        "resetAt": r.resets.first().map(|x| x.at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
                        "pace": r.pace.map(|p| format!("{:?}", p).to_lowercase()),
                        "members": r.members.into_iter().map(|m| {
                            json!({
                                "label": m.account.display_name.unwrap_or(m.account.id),
                                "remainingPercent": m.window.left_percent(),
                                "resetAt": m.window.resets_at.map(|x| x.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
                                "plan": m.account.plan,
                                "evenPaceLeft": m.window.even_pace_left(now),
                                "refillPercent": m.refill_percent,
                                "resetCredits": m.account.reset_credits.as_ref().map(|c| c.available_count)
                            })
                        }).collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "driver": g.driver,
                "label": provider_label(&g.driver),
                "rows": rows
            })
        })
        .collect();
    let links = s
        .links
        .iter()
        .filter(|l| l.url.starts_with("https://") || l.url.starts_with("http://"))
        .map(|l| {
            json!({
                "label": l.label,
                "url": l.url,
                "message": l.message,
                "accounts": l.accounts
            })
        })
        .collect();
    LimitsReport {
        groups,
        notices: s.notices,
        checked_at: s
            .accounts
            .iter()
            .map(|a| a.checked_at)
            .max()
            .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        links,
        read_failed,
        skipped_files: s.skipped_files,
    }
}

pub fn connection_for(
    live: limits::LiveSnapshot,
    online: bool,
    has_data: bool,
    server_port: Option<u16>,
) -> ConnectionState {
    let (status, message) = match &live.status {
        limits::LiveStatus::Live => ("connected", "Connected to T3 Code".to_owned()),
        limits::LiveStatus::Connecting => ("connecting", "Connecting to T3 Code…".to_owned()),
        limits::LiveStatus::Retrying => (
            "retrying",
            "Reconnecting. Showing the last available data.".to_owned(),
        ),
        limits::LiveStatus::Failed(message) => ("failed", message.clone()),
        _ if !online => (
            "offline",
            "Open T3 Code to connect. Showing saved data when present.".to_owned(),
        ),
        _ => (
            "cacheOnly",
            "Showing saved data. Connect to receive live limits.".to_owned(),
        ),
    };
    ConnectionState {
        status: status.into(),
        message,
        connected: live.status == limits::LiveStatus::Live,
        paired: live.has_credential,
        has_data,
        port: server_port.map(i64::from).unwrap_or(0),
        updated_at: iso(live.updated_at),
    }
}

// Environment discovery
pub fn fixture_or_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.trim().is_empty())
}

pub fn state_path() -> PathBuf {
    fixture_or_env("T3_BAR_FIXTURE_STATE")
        .map(PathBuf::from)
        .or_else(|| fixture_or_env("T3_CODE_BAR_STATE_DIR").map(PathBuf::from))
        .unwrap_or_else(|| threads::Configuration::from_environment(None).base_dir)
}

pub fn parse_now() -> Option<DateTime<Utc>> {
    fixture_or_env("T3_BAR_FROZEN_NOW")
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc))
}

pub fn parse_tz() -> Tz {
    fixture_or_env("T3_BAR_TIMEZONE")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            iana_time_zone::get_timezone()
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(chrono_tz::UTC)
        })
}

pub fn usage_period(raw: &str) -> usage::Period {
    match raw {
        "7d" => usage::Period::Week,
        "30d" => usage::Period::Month,
        "90d" => usage::Period::Quarter,
        _ => usage::Period::Day,
    }
}

pub fn panel_height(raw: f64) -> f64 {
    raw.clamp(80.0, 560.0)
}

pub fn settings_path(base: &Path) -> PathBuf {
    fixture_or_env("T3_BAR_FIXTURE_SETTINGS")
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join("tauri-bar-settings.json"))
}

#[derive(Default)]
pub struct FixtureCredentials(pub Mutex<Option<limits::Credential>>);
impl limits::CredentialStore for FixtureCredentials {
    fn load(&self) -> Result<Option<limits::Credential>, limits::ConnectionError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, cred: &limits::Credential) -> Result<(), limits::ConnectionError> {
        *self.0.lock().unwrap() = Some(cred.clone());
        Ok(())
    }
    fn delete(&self) -> Result<(), limits::ConnectionError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

pub struct FixtureFileCredentials(pub PathBuf);
impl limits::CredentialStore for FixtureFileCredentials {
    fn load(&self) -> Result<Option<limits::Credential>, limits::ConnectionError> {
        match std::fs::read(&self.0) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| limits::ConnectionError::Storage),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(limits::ConnectionError::Storage),
        }
    }
    fn save(&self, cred: &limits::Credential) -> Result<(), limits::ConnectionError> {
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let bytes = serde_json::to_vec(cred).map_err(|_| limits::ConnectionError::Storage)?;
        let tmp = self.0.with_extension("json.tmp");
        std::fs::write(&tmp, &bytes).map_err(|_| limits::ConnectionError::Storage)?;
        std::fs::rename(&tmp, &self.0).map_err(|_| limits::ConnectionError::Storage)
    }
    fn delete(&self) -> Result<(), limits::ConnectionError> {
        match std::fs::remove_file(&self.0) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(limits::ConnectionError::Storage),
        }
    }
}

pub fn credential_store(
    fixture: bool,
    namespace: Option<&str>,
) -> Result<Arc<dyn limits::CredentialStore>, String> {
    match (fixture, namespace) {
        (false, None) => Ok(Arc::new(limits::KeyringStore::production())),
        (true, None) => Ok(Arc::new(FixtureCredentials::default())),
        (true, Some(raw)) => {
            let id = uuid::Uuid::parse_str(raw)
                .map_err(|_| "Fixture keyring namespace must be a UUID")?;
            if id.to_string() != raw || id.is_nil() {
                return Err("Fixture keyring namespace must be a nonzero canonical UUID".into());
            }
            if let Some(dir) = fixture_or_env("T3_CODE_BAR_STATE_DIR") {
                return Ok(Arc::new(FixtureFileCredentials(
                    PathBuf::from(dir).join(format!(".fixture-keyring-{id}.json")),
                )));
            }
            Ok(Arc::new(limits::KeyringStore::new(
                format!("com.t3codebar.migration-probe.{id}"),
                "synthetic-only",
            )))
        }
        (false, Some(_)) => Err("Fixture keyring namespace requires fixture mode".into()),
    }
}

pub fn open_notification_thread<F: FnOnce(String) -> Result<(), String>>(
    config: &threads::Configuration,
    ids: &Mutex<HashSet<String>>,
    id: &str,
    open: F,
) -> Result<(), String> {
    if !ids.lock().map_err(|_| "threads unavailable")?.contains(id) {
        return Err("Notification thread is no longer available".into());
    }
    let url = config.thread_url(id, false).map_err(|e| e.to_string())?;
    open(url)
}

pub struct HostRuntime {
    pub config: threads::Configuration,
    pub settings: Mutex<SettingsStore>,
    pub snapshot: Mutex<AppSnapshot>,
    pub view: Mutex<ViewState>,
    pub ids: Mutex<HashSet<String>>,
    pub attention: Mutex<AttentionTracker>,
    pub favicons: Favicons,
    pub usage: Mutex<usage::UsageReader>,
    pub limits: Mutex<limits::LimitsReader>,
    pub frozen_now: Option<DateTime<Utc>>,
    pub timezone: Tz,
    pub fixture: bool,
    pub fixture_notifications: bool,
    pub connection: limits::LimitsConnection,
    /// Persistent runtime for the live limits stream. `resume()` spawns an
    /// eternal websocket task; spawning it on a short-lived runtime would
    /// cancel the task the moment that runtime is dropped, leaving the
    /// connection stuck in `Connecting` forever.
    pub connection_rt: tokio::runtime::Runtime,
    pub last_port: AtomicU16,
    pub connection_generation: AtomicU64,
    pub connection_gate: tokio::sync::Mutex<()>,
    pub thread_busy: AtomicBool,
    pub usage_busy: AtomicBool,
    pub limits_busy: AtomicBool,
    pub mode: SnapshotMode,
    pub sink: Arc<dyn HostEventSink>,
    pub visible: AtomicBool,
}

impl HostRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: threads::Configuration,
        mode: SnapshotMode,
        frozen_now: Option<DateTime<Utc>>,
        timezone: Tz,
        fixture: bool,
        fixture_notifications: bool,
        favicons_cache_dir: PathBuf,
        sink: Arc<dyn HostEventSink>,
    ) -> Arc<Self> {
        let creds = credential_store(
            fixture,
            fixture_or_env("T3_BAR_FIXTURE_KEYRING_NAMESPACE").as_deref(),
        )
        .unwrap_or_else(|_| Arc::new(FixtureCredentials::default()));

        let connection = limits::LimitsConnection::with_default_transport(creds);
        let _ = connection.restore_saved();
        let connection_rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("t3code-bar-connection")
            .enable_all()
            .build()
            .expect("failed to build connection runtime");
        let settings_file = settings_path(&config.base_dir);
        let settings = SettingsStore::load(settings_file.clone())
            .unwrap_or_else(|_| SettingsStore::defaults(settings_file));

        let mut initial_snapshot = AppSnapshot::default();
        if mode == SnapshotMode::HostBridge {
            initial_snapshot.update = None;
            initial_snapshot.summary = Some(ThreadSummary::default());
        }

        // Local provider roots (opencode/cursor/antigravity session data) must
        // be configured explicitly, or the reader only ever sees T3 Code's
        // saved scan cache — the desktop app sets them in its own host setup,
        // so the shared runtime has to do it here for the bridge to match.
        let home = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("/"));
        let mut usage_reader = usage::UsageReader::new(config.base_dir.clone());
        usage_reader.set_local_roots(usage::LocalRoots::discover(&config.base_dir, home));

        Arc::new(Self {
            favicons: Favicons::new(favicons_cache_dir),
            usage: Mutex::new(usage_reader),
            limits: Mutex::new(limits::LimitsReader::new()),
            frozen_now,
            timezone,
            fixture,
            fixture_notifications,
            connection,
            connection_rt,
            last_port: AtomicU16::new(0),
            connection_generation: AtomicU64::new(1),
            connection_gate: tokio::sync::Mutex::new(()),
            thread_busy: AtomicBool::new(false),
            usage_busy: AtomicBool::new(false),
            limits_busy: AtomicBool::new(false),
            mode,
            sink,
            visible: AtomicBool::new(false),
            config,
            settings: Mutex::new(settings),
            snapshot: Mutex::new(initial_snapshot),
            view: Mutex::new(ViewState::default()),
            ids: Mutex::new(HashSet::new()),
            attention: Mutex::new(AttentionTracker::default()),
        })
    }

    pub fn now(&self) -> DateTime<Utc> {
        self.frozen_now.unwrap_or_else(Utc::now)
    }

    pub fn snapshot(&self) -> AppSnapshot {
        self.snapshot.lock().unwrap().clone()
    }

    pub fn reload_settings(&self) {
        let settings_file = settings_path(&self.config.base_dir);
        if let Ok(store) = SettingsStore::load(settings_file) {
            let value = store.value().clone();
            *self.settings.lock().unwrap() = store;
            if let Ok(mut snap) = self.snapshot.lock() {
                if snap.settings != value {
                    snap.settings = value.clone();
                    self.sink.on_settings_changed(&value);
                    self.sink.on_snapshot_changed(&snap);
                }
            }
        }
    }

    pub fn set_setting(&self, key: SettingKey, value: bool) -> Result<(), String> {
        let mut store = self.settings.lock().unwrap();
        store.set(key, value).map_err(|e| e.to_string())?;
        let settings = store.value().clone();
        drop(store);

        if let Ok(mut snap) = self.snapshot.lock() {
            snap.settings = settings.clone();
            self.sink.on_settings_changed(&settings);
            self.sink.on_snapshot_changed(&snap);
        }
        Ok(())
    }

    pub fn set_view(
        &self,
        screen: Option<String>,
        metric: Option<String>,
        period: Option<String>,
        visible: Option<bool>,
    ) -> Result<(), String> {
        let mut v = self.view.lock().unwrap();
        if let Some(s) = screen {
            if !["threads", "usage", "settings"].contains(&s.as_str()) {
                return Err(format!("Invalid screen: {s}"));
            }
            v.screen = s;
        }
        if let Some(m) = metric {
            if !["cost", "tokens", "limits"].contains(&m.as_str()) {
                return Err(format!("Invalid metric: {m}"));
            }
            v.metric = m;
        }
        if let Some(p) = period {
            if !["24h", "7d", "30d", "90d"].contains(&p.as_str()) {
                return Err(format!("Invalid period: {p}"));
            }
            v.period = p;
        }
        if let Some(vis) = visible {
            v.visible = vis;
            self.visible.store(vis, Ordering::Release);
        }
        v.generation = v.generation.wrapping_add(1);
        Ok(())
    }

    pub fn refresh_threads_sync(&self) {
        if self.thread_busy.swap(true, Ordering::AcqRel) {
            return;
        }
        self.reload_settings();
        let now = self.now();
        let result = threads::read_threads(&self.config, now);
        match &result {
            Ok(value) => self.sink.on_tray_summary(&value.summary),
            Err(_) => self.sink.on_tray_summary(&ThreadSummary::new(&[], false)),
        }
        let mut alerts = Vec::new();
        if let Ok(mut snap) = self.snapshot.lock() {
            match result {
                Ok(v) => {
                    alerts = self.attention.lock().unwrap().update(
                        v.threads
                            .iter()
                            .map(|t| (t.id.as_str(), t.title.as_str(), t.status)),
                        snap.settings.notifications_muted
                            || !self.connection.snapshot().has_credential
                            || (self.fixture && !self.fixture_notifications),
                    );
                    snap.threads = v
                        .threads
                        .into_iter()
                        .map(|t| thread_item(t, &self.favicons, self.mode, self.fixture))
                        .collect();
                    snap.online = v.is_server_online;
                    snap.thread_error = None;
                    if self.mode == SnapshotMode::HostBridge {
                        snap.summary = Some(v.summary);
                    }
                    *self.ids.lock().unwrap() =
                        snap.threads.iter().map(|t| t.id.clone()).collect();
                }
                Err(e) => {
                    snap.online = false;
                    snap.thread_error = Some(e.to_string());
                    if self.mode == SnapshotMode::HostBridge {
                        snap.summary = Some(ThreadSummary::default());
                    }
                }
            }
            snap.now = now.to_rfc3339();
            snap.loading = false;
            self.refresh_connection(snap.online);
            snap.connection = connection_for(
                self.connection.snapshot(),
                snap.online,
                !snap.threads.is_empty()
                    || snap
                        .limits
                        .as_ref()
                        .is_some_and(|l| !l.groups.is_empty() || !l.links.is_empty())
                    || self.config.base_dir.join("statev2.sqlite").exists(),
                self.config.server_port().ok(),
            );
            self.sink.on_snapshot_changed(&snap);
        }
        if !alerts.is_empty() {
            self.sink.on_attention_alerts(&alerts);
        }
        self.thread_busy.store(false, Ordering::Release);
    }

    pub fn refresh_limits_sync(&self, read_cache: bool) {
        if self.limits_busy.swap(true, Ordering::AcqRel) {
            return;
        }
        let now = self.now();
        if let Ok(mut reader) = self.limits.lock() {
            let cached = if read_cache {
                Some(reader.read(&self.config.base_dir, now))
            } else {
                None
            };
            let merged = reader.merge_live(&self.connection);
            if let Ok(mut snap) = self.snapshot.lock() {
                if let Some(data) =
                    merged.or_else(|| cached.as_ref().and_then(|r| r.as_ref().ok()).cloned())
                {
                    snap.limits = Some(report_limits(
                        data,
                        now,
                        cached.as_ref().is_some_and(|r| r.is_err()),
                    ));
                } else if let Some(Err(_)) = cached {
                    if let Some(data) = &mut snap.limits {
                        data.read_failed = true;
                    }
                }
                self.sink.on_snapshot_changed(&snap);
            }
        }
        self.limits_busy.store(false, Ordering::Release);
    }

    pub fn refresh_visible_sync(&self) {
        if !self.visible.load(Ordering::Acquire) {
            return;
        }
        let view = self.view.lock().map(|v| v.clone()).unwrap_or_default();
        if view.screen != "usage" || view.metric == "limits" {
            return;
        }
        if self.usage_busy.swap(true, Ordering::AcqRel) {
            return;
        }
        let period = usage_period(&view.period);
        let now = self.now();
        let result = self
            .usage
            .lock()
            .map_err(|_| usage::UsageError::Unreadable)
            .and_then(|mut r| r.read(period, now, self.timezone));

        let current = self
            .view
            .lock()
            .map(|v| {
                v.generation == view.generation
                    && v.screen == "usage"
                    && v.metric != "limits"
                    && self.visible.load(Ordering::Acquire)
            })
            .unwrap_or(false);

        if current {
            if let Ok(v) = result {
                if let Ok(mut s) = self.snapshot.lock() {
                    s.usage = Some(report_usage(v, self.timezone));
                    s.usage_period = view.period.clone();
                    s.usage_error = None;
                    s.usage_loading = false;
                    self.sink.on_snapshot_changed(&s);
                }
            } else if let Ok(mut s) = self.snapshot.lock() {
                s.usage_loading = false;
                match result.unwrap_err() {
                    usage::UsageError::Missing => {
                        s.usage = None;
                        s.usage_error =
                            Some("No usage data yet. Open T3 Code to collect usage.".into());
                    }
                    usage::UsageError::UnsupportedVersion(_) => {
                        s.usage = None;
                        s.usage_error = Some(
                            "T3 Code's usage format has changed. Update T3 Code Bar to read it."
                                .into(),
                        );
                    }
                    usage::UsageError::Unreadable => {
                        s.usage_error = Some(
                            "Could not read usage. Showing the last successful scan.".into(),
                        );
                    }
                }
                self.sink.on_snapshot_changed(&s);
            }
        }
        self.usage_busy.store(false, Ordering::Release);
    }

    pub fn refresh_connection(&self, online: bool) {
        let port = if online {
            self.config.server_port().unwrap_or(0)
        } else {
            0
        };
        let previous = self.last_port.swap(port, Ordering::AcqRel);
        if previous != port {
            let generation = self.connection_generation.fetch_add(1, Ordering::AcqRel) + 1;
            // In synchronous/blocking or async runtime, gate the connection reconnect
            // We spawn a thread to handle reconnecting under gate
            let conn = self.connection.clone();
            let rt_handle = self.connection_rt.handle().clone();
            let last_gen = Arc::new(self.connection_generation.load(Ordering::Acquire));
            let _ = thread::spawn(move || {
                rt_handle.block_on(async {
                    if *last_gen != generation {
                        return;
                    }
                    if port == 0 {
                        conn.pause().await;
                    } else {
                        if previous != 0 {
                            conn.pause().await;
                        }
                        let _ = conn.sync_saved(port).await;
                    }
                });
            });
        } else {
            // Port unchanged: check if external CLI mutated saved credentials
            let conn = self.connection.clone();
            let rt_handle = self.connection_rt.handle().clone();
            let _ = thread::spawn(move || {
                rt_handle.block_on(async {
                    let _ = conn.sync_saved(port).await;
                })
            });
        }
    }

    pub fn tick(&self, tick_count: u64) {
        self.refresh_threads_sync();
        self.refresh_visible_sync();
        self.refresh_limits_sync(
            tick_count == 0
                || (tick_count.is_multiple_of(5) && self.visible.load(Ordering::Acquire)),
        );
    }

    pub fn start_poller(self: &Arc<Self>) {
        let this = self.clone();
        thread::spawn(move || {
            let _ = this.connection.restore_saved();
            let mut tick_count = 0u64;
            loop {
                this.tick(tick_count);
                tick_count = tick_count.wrapping_add(1);
                thread::sleep(Duration::from_secs(1));
            }
        });
    }
}
