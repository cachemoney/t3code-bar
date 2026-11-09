#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod platform_linux;
#[cfg(target_os = "macos")]
mod platform_macos;
#[cfg(target_os = "linux")]
use platform_linux as platform;
#[cfg(target_os = "macos")]
use platform_macos as platform;

mod favicons;
mod tray_status;
mod updater_adapter;
mod updates;

use bar_core::{
    attention::AttentionTracker,
    host::{
        connection_for as core_connection_for, credential_store, fixture_or_env,
        open_notification_thread, panel_height, parse_now, parse_tz, report_limits,
        report_usage, settings_path, state_path, thread_item as core_thread_item, usage_period,
        AppSnapshot, ConnectionState, FixtureCredentials, SnapshotMode, ThreadItem, UpdateState,
        ViewState,
    },
    limits,
    settings::{SettingKey, SettingsStore},
    threads, usage,
};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde_json::json;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};

static VISIBLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PANEL_HEIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(560);
static PANEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

struct RuntimeState {
    config: threads::Configuration,
    settings: Mutex<SettingsStore>,
    snapshot: Mutex<AppSnapshot>,
    view: Mutex<ViewState>,
    ids: Mutex<HashSet<String>>,
    attention: Mutex<AttentionTracker>,
    favicons: favicons::Favicons,
    usage: Mutex<usage::UsageReader>,
    limits: Mutex<limits::LimitsReader>,
    frozen_now: Option<DateTime<Utc>>,
    timezone: Tz,
    fixture: bool,
    fixture_notifications: bool,
    connection: limits::LimitsConnection,
    last_port: std::sync::atomic::AtomicU16,
    connection_generation: std::sync::atomic::AtomicU64,
    connection_gate: tokio::sync::Mutex<()>,
    thread_busy: std::sync::atomic::AtomicBool,
    usage_busy: std::sync::atomic::AtomicBool,
    limits_busy: std::sync::atomic::AtomicBool,
}
type SharedState = Arc<RuntimeState>;

fn now(state: &RuntimeState) -> DateTime<Utc> {
    state.frozen_now.unwrap_or_else(Utc::now)
}
pub fn record(event: &str, value: serde_json::Value) {
    eprintln!("[bar:{event}] {}", value);
}
fn record_failure(operation: &str, result: Result<(), String>) {
    if let Err(error) = result {
        record(
            "runtime_error",
            json!({"operation":operation,"error":error}),
        );
    }
}
fn thread_item(t: threads::ThreadItem, state: &RuntimeState) -> ThreadItem {
    core_thread_item(t, &state.favicons, SnapshotMode::Tauri, state.fixture)
}

fn connection_for(state: &RuntimeState, online: bool, has_data: bool) -> ConnectionState {
    core_connection_for(
        state.connection.snapshot(),
        online,
        has_data,
        state.config.server_port().ok(),
    )
}
fn emit_snapshot(app: &tauri::AppHandle, state: &RuntimeState) {
    if let Ok(s) = state.snapshot.lock().map(|x| x.clone()) {
        let _ = app.emit("bar-snapshot", s);
    }
}
fn refresh_connection(state: &SharedState, online: bool) {
    let port = if online {
        state.config.server_port().unwrap_or(0)
    } else {
        0
    };
    let previous = state
        .last_port
        .swap(port, std::sync::atomic::Ordering::AcqRel);
    if previous == port {
        return;
    }
    let state = state.clone();
    let generation = begin_connection_operation(&state);
    tauri::async_runtime::spawn(async move {
        let _gate = state.connection_gate.lock().await;
        if !connection_operation_current(&state, generation) {
            return;
        }
        let connection = &state.connection;
        if port == 0 {
            connection.pause().await;
        } else {
            if previous != 0 {
                connection.pause().await;
            }
            let _ = connection.reconnect_saved_if_available(port).await;
        }
    });
}
fn refresh_threads(app: &tauri::AppHandle, state: &SharedState) {
    if state
        .thread_busy
        .swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        return;
    }
    let app = app.clone();
    let state = state.clone();
    thread::spawn(move || {
        let result = threads::read_threads(&state.config, now(&state));
        match &result {
            Ok(value) => tray_status::update(&app, &value.summary),
            Err(_) => tray_status::update(&app, &threads::ThreadSummary::new(&[], false)),
        }
        let mut alerts = Vec::new();
        if let Ok(mut snap) = state.snapshot.lock() {
            match result {
                Ok(v) => {
                    alerts = state.attention.lock().unwrap().update(
                        v.threads
                            .iter()
                            .map(|t| (t.id.as_str(), t.title.as_str(), t.status)),
                        snap.settings.notifications_muted
                            || !state.connection.snapshot().has_credential
                            || (state.fixture && !state.fixture_notifications),
                    );
                    snap.threads = v
                        .threads
                        .into_iter()
                        .map(|t| thread_item(t, &state))
                        .collect();
                    snap.online = v.is_server_online;
                    snap.thread_error = None;
                    *state.ids.lock().unwrap() =
                        snap.threads.iter().map(|t| t.id.clone()).collect();
                }
                Err(e) => {
                    snap.online = false;
                    snap.thread_error = Some(e.to_string());
                }
            }
            snap.now = now(&state).to_rfc3339();
            snap.loading = false;
            refresh_connection(&state, snap.online);
            snap.connection = connection_for(
                &state,
                snap.online,
                !snap.threads.is_empty()
                    || snap
                        .limits
                        .as_ref()
                        .is_some_and(|l| !l.groups.is_empty() || !l.links.is_empty())
                    || state.config.base_dir.join("statev2.sqlite").exists(),
            );
        }
        for alert in alerts {
            use tauri_plugin_notification::NotificationExt;
            let notification = app
                .notification()
                .builder()
                .title(alert.title())
                .body(&alert.body)
                .extra("threadId", alert.thread_id);
            #[cfg(target_os = "macos")]
            let notification = notification.sound("NSUserNotificationDefaultSoundName");
            if let Err(error) = notification.show() {
                record("notification_error", json!({"error":error.to_string()}));
            }
        }
        state
            .thread_busy
            .store(false, std::sync::atomic::Ordering::Release);
        emit_snapshot(&app, &state);
    });
}
fn refresh_limits(app: &tauri::AppHandle, state: &SharedState, read_cache: bool) {
    if state
        .limits_busy
        .swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        return;
    }
    let app = app.clone();
    let state = state.clone();
    thread::spawn(move || {
        if let Ok(mut reader) = state.limits.lock() {
            let cached = if read_cache {
                Some(reader.read(&state.config.base_dir, now(&state)))
            } else {
                None
            };
            let merged = reader.merge_live(&state.connection);
            if let Ok(mut snap) = state.snapshot.lock() {
                if let Some(data) =
                    merged.or_else(|| cached.as_ref().and_then(|r| r.as_ref().ok()).cloned())
                {
                    snap.limits = Some(report_limits(
                        data,
                        now(&state),
                        cached.as_ref().is_some_and(|r| r.is_err()),
                    ));
                } else if let Some(Err(_)) = cached {
                    if let Some(data) = &mut snap.limits {
                        data.read_failed = true;
                    }
                }
            }
        }
        state
            .limits_busy
            .store(false, std::sync::atomic::Ordering::Release);
        emit_snapshot(&app, &state);
    });
}
fn refresh_visible(app: &tauri::AppHandle, state: &SharedState) {
    if !VISIBLE.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let view = state.view.lock().map(|v| v.clone()).unwrap_or_default();
    if view.screen != "usage" {
        return;
    }
    if view.metric != "limits" {
        if state
            .usage_busy
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return;
        }
        let period = usage_period(&view.period);
        let app = app.clone();
        let state = state.clone();
        thread::spawn(move || {
            let result = state
                .usage
                .lock()
                .map_err(|_| usage::UsageError::Unreadable)
                .and_then(|mut r| r.read(period, now(&state), state.timezone));
            let current = state
                .view
                .lock()
                .map(|v| {
                    v.generation == view.generation
                        && v.screen == "usage"
                        && v.metric != "limits"
                        && VISIBLE.load(std::sync::atomic::Ordering::Acquire)
                })
                .unwrap_or(false);
            if current {
                if let Ok(v) = result {
                    if let Ok(mut s) = state.snapshot.lock() {
                        s.usage = Some(report_usage(v, state.timezone));
                        s.usage_period = view.period.clone();
                        s.usage_error = None;
                        s.usage_loading = false;
                    }
                } else if let Ok(mut s) = state.snapshot.lock() {
                    s.usage_loading = false;
                    match result.unwrap_err() {
                        usage::UsageError::Missing => {
                            s.usage = None;
                            s.usage_error =
                                Some("No usage data yet. Open T3 Code to collect usage.".into());
                        }
                        usage::UsageError::UnsupportedVersion(_) => {
                            s.usage = None;
                            s.usage_error = Some("T3 Code's usage format has changed. Update T3 Code Bar to read it.".into());
                        }
                        usage::UsageError::Unreadable => {
                            s.usage_error = Some(
                                "Could not read usage. Showing the last successful scan.".into(),
                            );
                        }
                    }
                }
            }
            state
                .usage_busy
                .store(false, std::sync::atomic::Ordering::Release);
            emit_snapshot(&app, &state);
        });
    }
}
fn poller(app: tauri::AppHandle, state: SharedState) {
    thread::spawn(move || {
        {
            let _gate = state.connection_gate.blocking_lock();
            if let Err(error) = state.connection.restore_saved() {
                record(
                    "connection_restore_error",
                    json!({"error": error.to_string()}),
                );
            }
        }
        let mut tick = 0u64;
        loop {
            refresh_threads(&app, &state);
            refresh_visible(&app, &state);
            refresh_limits(
                &app,
                &state,
                tick == 0
                    || (tick.is_multiple_of(5)
                        && VISIBLE.load(std::sync::atomic::Ordering::Acquire)),
            );
            tick = tick.wrapping_add(1);
            thread::sleep(Duration::from_secs(1));
        }
    });
}
pub fn panel_opened(app: &tauri::AppHandle) {
    if !VISIBLE.swap(true, std::sync::atomic::Ordering::AcqRel) {
        if let Some(state) = app.try_state::<SharedState>() {
            if let Ok(mut view) = state.view.lock() {
                view.screen = "threads".into();
                view.generation = view.generation.wrapping_add(1);
            }
        }
        let _ = app.emit("bar-opened", ());
        updates::show_pending(app);
    }
}
pub fn panel_closed(app: &tauri::AppHandle) {
    VISIBLE.store(false, std::sync::atomic::Ordering::Release);
    if let Some(state) = app.try_state::<SharedState>() {
        if let Ok(mut view) = state.view.lock() {
            view.screen = "threads".into();
            view.generation = view.generation.wrapping_add(1);
        }
    }
}

#[tauri::command]
fn get_snapshot(state: State<'_, SharedState>) -> Result<AppSnapshot, String> {
    state
        .snapshot
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "snapshot unavailable".into())
}
#[tauri::command]
fn set_view(
    screen: Option<String>,
    metric: Option<String>,
    period: Option<String>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut v = state.view.lock().map_err(|_| "view unavailable")?;
    if let Some(s) = screen {
        if ["threads", "usage", "settings"].contains(&s.as_str()) {
            v.screen = s;
        }
    }
    if let Some(m) = metric {
        if ["cost", "tokens", "limits"].contains(&m.as_str()) {
            v.metric = m;
        }
    }
    if let Some(p) = period {
        if ["24h", "7d", "30d", "90d"].contains(&p.as_str()) {
            v.period = p;
        }
    }
    v.generation = v.generation.wrapping_add(1);
    Ok(())
}
#[tauri::command]
fn resize_panel(height: f64, window: tauri::Window) -> Result<(), String> {
    if !height.is_finite() {
        return Err("Invalid panel height".into());
    }
    let h = panel_height(height).ceil() as u32;
    if PANEL_HEIGHT.swap(h, std::sync::atomic::Ordering::AcqRel) == h {
        return Ok(());
    }
    platform::resize(&window, h)
}
#[tauri::command]
fn hide_panel(window: tauri::Window) -> Result<(), String> {
    platform::hide(&window)
}
#[tauri::command]
fn set_setting(
    key: SettingKey,
    value: bool,
    state: State<'_, SharedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut settings = state.settings.lock().map_err(|_| "settings unavailable")?;
    settings.set(key, value).map_err(|e| e.to_string())?;
    let next = settings.value().clone();
    drop(settings);
    if let Ok(mut snap) = state.snapshot.lock() {
        snap.settings = next;
    }
    let _ = app.emit("bar-settings", ());
    emit_snapshot(&app, &state);
    Ok(())
}
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
#[tauri::command]
fn thread_action(
    id: String,
    action: String,
    state: State<'_, SharedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if !state
        .ids
        .lock()
        .map_err(|_| "threads unavailable")?
        .contains(&id)
    {
        return Err("Unknown thread".into());
    }
    let url = state
        .config
        .thread_url(&id, action == "browser")
        .map_err(|e| e.to_string())?;
    match action.as_str() {
        "copy" => {
            use tauri_plugin_clipboard_manager::ClipboardExt;
            app.clipboard()
                .write_text(&url)
                .map_err(|e| e.to_string())?;
        }
        "open" | "browser" => {
            use tauri_plugin_opener::OpenerExt;
            app.opener()
                .open_url(url, None::<String>)
                .map_err(|e| e.to_string())?;
        }
        _ => return Err("Unknown thread action".into()),
    }
    Ok(())
}
#[tauri::command]
fn new_thread(app: tauri::AppHandle, _state: State<'_, SharedState>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url("t3code://new", None::<String>)
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn get_preview(
    id: String,
    state: State<'_, SharedState>,
) -> Result<Option<threads::ThreadPreview>, String> {
    if !state
        .snapshot
        .lock()
        .map_err(|_| "snapshot unavailable")?
        .settings
        .shows_previews
    {
        return Ok(None);
    }
    if !state
        .ids
        .lock()
        .map_err(|_| "threads unavailable")?
        .contains(&id)
    {
        return Err("Unknown thread".into());
    }
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        if state.fixture {
            let delay = fixture_or_env("T3_BAR_FIXTURE_PREVIEW_DELAY_MS")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0)
                .min(5000);
            record("fixture_preview_started", json!({"id":id,"delayMs":delay}));
            thread::sleep(Duration::from_millis(delay));
        }
        let result = threads::read_preview(&state.config, &id).map_err(|e| e.to_string());
        if state.fixture {
            record(
                "fixture_preview_completed",
                json!({"id":id,"ok":result.is_ok()}),
            );
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn open_limit_link(
    url: String,
    state: State<'_, SharedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let snap = state.snapshot.lock().map_err(|_| "snapshot unavailable")?;
    let allowed = snap
        .limits
        .as_ref()
        .map(|l| {
            l.links
                .iter()
                .any(|v| v.get("url").and_then(|u| u.as_str()) == Some(url.as_str()))
        })
        .unwrap_or(false);
    if !allowed {
        return Err("Link is not in the current limits snapshot".into());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<String>)
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<(), String> {
    updates::check(&app, bar_updater::CheckKind::Manual).await;
    Ok(())
}
#[tauri::command]
async fn update_prompt_shown(app: tauri::AppHandle, offer_id: u64, version: String) {
    updates::acknowledge_prompt(&app, offer_id, &version).await;
}
#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    offer_id: u64,
    version: String,
) -> Result<(), String> {
    updates::install(&app, offer_id, &version).await
}
fn begin_connection_operation(state: &RuntimeState) -> u64 {
    state
        .connection_generation
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        .wrapping_add(1)
}
fn connection_operation_current(state: &RuntimeState, generation: u64) -> bool {
    state
        .connection_generation
        .load(std::sync::atomic::Ordering::Acquire)
        == generation
}
#[tauri::command]
async fn connect(
    token: String,
    state: State<'_, SharedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let generation = begin_connection_operation(&state);
    let port = state.config.server_port().map_err(|e| e.to_string())?;
    let _gate = state.connection_gate.lock().await;
    if !connection_operation_current(&state, generation) {
        return Ok(());
    }
    let result = state
        .connection
        .connect(port, &token)
        .await
        .map_err(|e| e.to_string());
    refresh_threads(&app, &state);
    result
}
#[tauri::command]
async fn reconnect(state: State<'_, SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    let generation = begin_connection_operation(&state);
    let port = state.config.server_port().map_err(|e| e.to_string())?;
    let _gate = state.connection_gate.lock().await;
    if !connection_operation_current(&state, generation) {
        return Ok(());
    }
    let result = state
        .connection
        .reconnect_saved(port)
        .await
        .map_err(|e| e.to_string());
    refresh_threads(&app, &state);
    result
}
#[tauri::command]
async fn disconnect(state: State<'_, SharedState>, app: tauri::AppHandle) -> Result<(), String> {
    begin_connection_operation(&state);
    let _gate = state.connection_gate.lock().await;
    let result = state
        .connection
        .disconnect()
        .await
        .map_err(|e| e.to_string());
    refresh_limits(&app, &state, true);
    refresh_threads(&app, &state);
    result
}

fn make_tray(app: &mut tauri::App) -> tauri::Result<()> {
    if cfg!(target_os = "linux") {
        return Ok(());
    }
    let show = MenuItem::with_id(app, "show", "Show T3 Code Bar", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit T3 Code Bar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &quit])?;
    TrayIconBuilder::with_id("bar-tray")
        .icon(tauri::include_image!("icons/icon.png"))
        .menu(&menu)
        .tooltip("T3 Code Bar")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                platform::toggle(app);
            }
        })
        .on_menu_event(|app, e| match e.id.as_ref() {
            "show" => {
                record_failure("tray_show", platform::show(app));
            }
            "settings" => {
                record_failure("tray_settings_show", platform::show(app));
                record_failure(
                    "tray_settings_event",
                    app.emit("bar-settings", ()).map_err(|e| e.to_string()),
                );
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    tray_status::update(app.handle(), &threads::ThreadSummary::new(&[], false));
    Ok(())
}
fn main() {
    platform::prepare_runtime();
    let base = state_path();
    let config = threads::Configuration {
        base_dir: base.clone(),
    };
    let fixture = fixture_or_env("T3_BAR_FIXTURE_STATE").is_some();
    let settings_file = settings_path(&base);
    let (settings, startup_error) = match SettingsStore::load(settings_file.clone()) {
        Ok(settings) => (settings, None),
        Err(error) => {
            eprintln!(
                "Cannot load settings; keeping the existing file and using defaults: {error}"
            );
            (SettingsStore::defaults(settings_file), Some("Settings could not be loaded. Using defaults for this session; the original file is preserved and setting changes cannot be saved.".into()))
        }
    };
    let namespace = fixture_or_env("T3_BAR_FIXTURE_KEYRING_NAMESPACE");
    let store = match credential_store(fixture, namespace.as_deref()) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("Cannot configure credential storage: {error}");
            std::process::exit(1);
        }
    };
    let mut usage_reader = usage::UsageReader::new(base.clone());
    if !fixture {
        if let Some(home) = fixture_or_env("HOME") {
            usage_reader.set_local_roots(usage::LocalRoots::discover(&base, PathBuf::from(home)));
        }
    }

    let state = Arc::new(RuntimeState {
        config: config.clone(),
        snapshot: Mutex::new(AppSnapshot {
            startup_error,
            settings: settings.value().clone(),
            ..Default::default()
        }),
        settings: Mutex::new(settings),
        view: Mutex::new(ViewState::default()),
        ids: Mutex::new(HashSet::new()),
        attention: Mutex::new(AttentionTracker::default()),
        favicons: Default::default(),
        usage: Mutex::new(usage_reader),
        limits: Mutex::new(limits::LimitsReader::new()),
        frozen_now: parse_now(),
        timezone: parse_tz(),
        fixture,
        fixture_notifications: fixture
            && fixture_or_env("T3_BAR_FIXTURE_NOTIFICATIONS").as_deref() == Some("1"),
        connection: limits::LimitsConnection::with_default_transport(store),
        last_port: Default::default(),
        connection_generation: Default::default(),
        connection_gate: tokio::sync::Mutex::new(()),
        thread_busy: Default::default(),
        usage_busy: Default::default(),
        limits_busy: Default::default(),
    });
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            record_failure("single_instance_show", platform::show(app));
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            set_view,
            resize_panel,
            hide_panel,
            set_setting,
            quit_app,
            thread_action,
            new_thread,
            get_preview,
            open_limit_link,
            check_for_updates,
            install_update,
            update_prompt_shown,
            connect,
            reconnect,
            disconnect
        ]);
    let result = builder
        .setup(move |app| {
            make_tray(app)?;
            platform::initialize(app)?;
            use tauri_plugin_notification::NotificationExt;
            let handle = app.handle().clone();
            app.notification().on_action(move |event| {
                if event.action_id() != "tap" {
                    return;
                }
                let Some(id) = event
                    .notification()
                    .and_then(|n| n.extra().get("threadId"))
                    .and_then(|v| v.as_str())
                else {
                    return;
                };
                let state = handle.state::<SharedState>();
                if state.fixture {
                    if state.fixture_notifications {
                        record("fixture_notification_tap", json!({"threadId":id}));
                    }
                    return;
                }
                if let Err(error) = open_notification_thread(&state.config, &state.ids, id, |url| {
                    use tauri_plugin_opener::OpenerExt;
                    handle
                        .opener()
                        .open_url(url, None::<String>)
                        .map_err(|e| e.to_string())
                }) {
                    record("notification_open_error", json!({"error":error}));
                }
            })?;
            updates::initialize(app.handle());
            if state.fixture && fixture_or_env("T3_BAR_FIXTURE_OPEN").as_deref() == Some("1") {
                let handle = app.handle().clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(500));
                    let target = handle.clone();
                    let _ = handle.run_on_main_thread(move || {
                        let _ = platform::show(&target);
                    });
                });
            }
            poller(app.handle().clone(), state.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                record_failure("close_hide", platform::hide(window));
            }
            platform::on_window_event(window, event);
        })
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("T3 Code Bar failed to start: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_routes_only_current_threads_and_preserves_opener_errors() {
        let directory =
            std::env::temp_dir().join(format!("t3-notification-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("environment-id"), "fixture-environment").unwrap();
        let config = threads::Configuration {
            base_dir: directory.clone(),
        };
        let ids = Mutex::new(HashSet::from(["thread-1".into()]));
        let result = open_notification_thread(&config, &ids, "missing", |_| {
            panic!("unknown thread opened")
        });
        assert_eq!(
            result,
            Err("Notification thread is no longer available".into())
        );
        let mut routed = None;
        open_notification_thread(&config, &ids, "thread-1", |url| {
            routed = Some(url);
            Ok(())
        })
        .unwrap();
        assert_eq!(routed, Some(config.thread_url("thread-1", false).unwrap()));
        assert_eq!(
            open_notification_thread(&config, &ids, "thread-1", |_| Err(
                "opener unavailable".into()
            )),
            Err("opener unavailable".into())
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn usage_timeline_metadata_survives_host_mapping() {
        use chrono::TimeZone;
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/usage/state");
        let dataset =
            usage::decode_cache(&std::fs::read(root.join("usage-scan-cache-v11.json")).unwrap())
                .unwrap();
        let now = Utc.with_ymd_and_hms(2026, 11, 6, 9, 8, 0).unwrap();
        for period in [
            usage::Period::Day,
            usage::Period::Week,
            usage::Period::Month,
            usage::Period::Quarter,
        ] {
            let window = usage::Window::new(period, now, chrono_tz::Asia::Ho_Chi_Minh);
            let mut source =
                usage::UsageReport::build(&dataset, &window, &usage::PriceBook::load(&root));
            source.malformed_records = 2;
            source.rejected_files = 1;
            source.cache_modified = Some(now);
            source.scan_cache_missing = false;
            source.local_sources.push(usage::LocalStatus {
                kind: "opencode".into(),
                state: "partial".into(),
                checked_at: now,
                data_updated_at: None,
                failed_files: 1,
            });
            let expected = source.timeline.clone();
            let mapped =
                serde_json::to_value(report_usage(source, chrono_tz::Asia::Ho_Chi_Minh)).unwrap();
            assert_eq!(mapped["malformedRecords"], 2);
            assert_eq!(mapped["rejectedFiles"], 1);
            assert_eq!(mapped["cacheModified"], "2026-11-06T09:08:00.000Z");
            assert_eq!(mapped["scanCacheMissing"], false);
            assert_eq!(mapped["localSourcesPartial"], true);
            assert_eq!(mapped["resolution"], expected.resolution);
            assert_eq!(mapped["timeZone"], expected.timezone);
            assert_eq!(mapped["untilMs"], expected.until_ms);
            assert_eq!(
                mapped["timeline"].as_array().unwrap().len(),
                expected.columns.len()
            );
            for (column, original) in mapped["timeline"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected.columns)
            {
                assert_eq!(column["day"], original.day);
                assert_eq!(
                    column["hourStartMs"],
                    serde_json::json!(original.hour_start_ms)
                );
            }
        }
    }

    #[test]
    fn runtime_destination_change_invalidates_an_inflight_connection() {
        use std::sync::atomic::Ordering;
        let root = std::env::temp_dir().join(format!("t3-bar-host-test-{}", uuid::Uuid::new_v4()));
        let settings = SettingsStore::load(root.join("settings.json")).unwrap();
        let state = Arc::new(RuntimeState {
            config: threads::Configuration {
                base_dir: root.clone(),
            },
            settings: Mutex::new(settings),
            snapshot: Mutex::new(AppSnapshot::default()),
            view: Mutex::new(ViewState::default()),
            ids: Mutex::new(HashSet::new()),
            attention: Mutex::new(AttentionTracker::default()),
            favicons: Default::default(),
            usage: Mutex::new(usage::UsageReader::new(root.clone())),
            limits: Mutex::new(limits::LimitsReader::new()),
            frozen_now: None,
            timezone: chrono_tz::UTC,
            fixture: true,
            fixture_notifications: false,
            connection: limits::LimitsConnection::with_default_transport(Arc::new(
                FixtureCredentials::default(),
            )),
            last_port: std::sync::atomic::AtomicU16::new(49216),
            connection_generation: std::sync::atomic::AtomicU64::new(1),
            connection_gate: tokio::sync::Mutex::new(()),
            thread_busy: Default::default(),
            usage_busy: Default::default(),
            limits_busy: Default::default(),
        });
        refresh_connection(&state, false);
        assert_eq!(state.connection_generation.load(Ordering::Acquire), 2);
        assert!(!connection_operation_current(&state, 1));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn panel_height_is_bounded_to_native_panel_contract() {
        assert_eq!(panel_height(20.0), 80.0);
        assert_eq!(panel_height(320.0), 320.0);
        assert_eq!(panel_height(900.0), 560.0);
    }
    #[test]
    fn fixture_keyring_cannot_select_production_or_arbitrary_namespaces() {
        assert!(credential_store(false, Some("com.t3code.bar.tauri.limits")).is_err());
        assert!(credential_store(true, Some("com.t3code.bar.tauri.limits")).is_err());
        assert!(credential_store(true, Some("00000000-0000-0000-0000-000000000000")).is_err());
        let namespace = uuid::Uuid::new_v4().to_string();
        assert!(credential_store(false, Some(&namespace)).is_err());
        assert!(credential_store(true, Some(&namespace)).is_ok());
    }
    #[test]
    fn usage_period_mapping_matches_frontend_labels() {
        assert_eq!(usage_period("24h"), usage::Period::Day);
        assert_eq!(usage_period("7d"), usage::Period::Week);
        assert_eq!(usage_period("30d"), usage::Period::Month);
        assert_eq!(usage_period("90d"), usage::Period::Quarter);
        assert_eq!(usage_period("unexpected"), usage::Period::Day);
    }
}
