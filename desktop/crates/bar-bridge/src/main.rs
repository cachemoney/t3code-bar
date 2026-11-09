use bar_core::{
    host::{parse_now, parse_tz, HostRuntime, NoopEventSink, SnapshotMode},
    projection::{StateFilePayload, StateProjection},
    threads::{Configuration, ThreadSummary},
    usage::Period,
};
use std::{
    collections::HashMap,
    env, fs,
    io::{Read, Write},
    os::unix::io::AsRawFd,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

fn resolve_state_dir(cli_dir: Option<&str>) -> PathBuf {
    if let Some(d) = cli_dir {
        return PathBuf::from(d);
    }
    if let Ok(d) = env::var("T3_CODE_BAR_STATE_DIR") {
        if !d.trim().is_empty() {
            return PathBuf::from(d);
        }
    }
    if let Ok(d) = env::var("XDG_STATE_HOME") {
        if !d.trim().is_empty() {
            return PathBuf::from(d).join("t3code-bar");
        }
    }
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/state/t3code-bar")
}

fn write_atomic(dir: &Path, filename: &str, content: &str) -> std::io::Result<()> {
    let temp_name = format!(".{filename}.tmp.{}", std::process::id());
    let temp_path = dir.join(temp_name);
    let target_path = dir.join(filename);

    {
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.flush()?;
    }
    fs::rename(temp_path, target_path)
}

fn acquire_flock(state_dir: &Path) -> Result<fs::File, ()> {
    let lock_path = state_dir.join(".watch.lock");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .map_err(|e| {
            eprintln!("Failed to open lock file: {}", e);
        })?;

    let res = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if res != 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EWOULDBLOCK) || err.raw_os_error() == Some(libc::EAGAIN) {
            // Another watch is already running. Contention loser exits 0 cleanly.
            std::process::exit(0);
        }
        eprintln!("flock error: {}", err);
        return Err(());
    }
    Ok(file)
}

fn run_snapshot(state_dir: &Path) -> Result<(), String> {
    let favicons_dir = state_dir.join("favicons");
    let config = Configuration::from_environment(None);
    let timezone = parse_tz();
    let frozen_now = parse_now();

    let fixture_notifications = env::var("T3_BAR_FIXTURE_NOTIFICATIONS").is_ok();
    let runtime = HostRuntime::new(
        config,
        SnapshotMode::HostBridge,
        frozen_now,
        timezone,
        env::var("T3_BAR_FIXTURE_STATE").is_ok(),
        fixture_notifications,
        favicons_dir,
        Arc::new(bar_bridge::notification::BridgeEventSink),
    );

    runtime.refresh_threads_sync();
    runtime.refresh_limits_sync(true);

    let snapshot = runtime.snapshot();
    let json_str = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    println!("{}", json_str);
    Ok(())
}

fn run_watch(state_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(state_dir).map_err(|e| format!("failed to create state dir: {e}"))?;
    let _lock = acquire_flock(state_dir).map_err(|_| "failed to acquire lock".to_string())?;

    // 1. Write initial state.json with loading: true before first poll completes
    let initial_payload = StateFilePayload {
        now: chrono::Utc::now().to_rfc3339(),
        loading: true,
        online: false,
        startup_error: None,
        thread_error: None,
        version: env!("CARGO_PKG_VERSION").to_string(),
        summary: ThreadSummary::default(),
        connection: bar_core::host::ConnectionState {
            status: "unavailable".into(),
            message: "T3 Code is unavailable. Showing saved data when present.".into(),
            connected: false,
            paired: false,
            has_data: false,
            port: 0,
            updated_at: None,
        },
        settings: bar_core::settings::Settings::default(),
        threads: vec![],
    };
    let initial_json = serde_json::to_string_pretty(&initial_payload).unwrap() + "\n";
    write_atomic(state_dir, "state.json", &initial_json).map_err(|e| e.to_string())?;

    if let Ok(delay_str) = env::var("T3_BAR_FIXTURE_POLL_DELAY_MS") {
        if let Ok(ms) = delay_str.parse::<u64>() {
            thread::sleep(Duration::from_millis(ms));
        }
    }

    let favicons_dir = state_dir.join("favicons");
    let config = Configuration::from_environment(None);
    let timezone = parse_tz();
    let frozen_now = parse_now();

    let fixture_notifications = env::var("T3_BAR_FIXTURE_NOTIFICATIONS").is_ok();
    let runtime = HostRuntime::new(
        config,
        SnapshotMode::HostBridge,
        frozen_now,
        timezone,
        env::var("T3_BAR_FIXTURE_STATE").is_ok(),
        fixture_notifications,
        favicons_dir,
        Arc::new(bar_bridge::notification::BridgeEventSink),
    );

    let mut projection = StateProjection::new();

    // Spawn stdin-EOF watchdog
    let (stdin_tx, stdin_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = [0u8; 1];
        let mut stdin = std::io::stdin();
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => {
                    let _ = stdin_tx.send(());
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    let _ = stdin_tx.send(());
                    break;
                }
                Ok(_) => {}
            }
        }
    });

    let parent_pid = unsafe { libc::getppid() };
    let mut tick = 0u64;

    loop {
        // Watchdog checks: parent shell PID death
        if parent_pid > 1 {
            let ret = unsafe { libc::kill(parent_pid, 0) };
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ESRCH) {
                    break;
                }
            }
        }

        // Watchdog checks: stdin EOF
        if stdin_rx.try_recv().is_ok() {
            break;
        }

        // Re-read view.json if modified externally
        let view_path = state_dir.join("view.json");
        if view_path.exists() {
            if let Ok(content) = fs::read_to_string(&view_path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    let screen = v.get("screen").and_then(|s| s.as_str()).map(str::to_string);
                    let metric = v.get("metric").and_then(|s| s.as_str()).map(str::to_string);
                    let period = v.get("period").and_then(|s| s.as_str()).map(str::to_string);
                    let visible = v.get("visible").and_then(|s| s.as_bool());
                    let _ = runtime.set_view(screen, metric, period, visible);
                }
            }
        }

        // Refresh threads on 1s cadence
        runtime.refresh_threads_sync();

        // Refresh limits on tick 0 or every 5th tick
        runtime.refresh_limits_sync(tick == 0 || tick % 5 == 0);

        // Usage gating: read usage reports if visible and on usage screen and metric is not limits
        let view = runtime.view.lock().map(|v| v.clone()).unwrap_or_default();
        let should_scan_usage = view.visible && view.screen == "usage" && view.metric != "limits";
        let mut usage_reports = HashMap::new();
        if should_scan_usage {
            let now = runtime.now();
            if let Ok(mut u) = runtime.usage.lock() {
                for p in [Period::Day, Period::Week, Period::Month, Period::Quarter] {
                    if let Ok(r) = u.read(p, now, timezone) {
                        usage_reports.insert(p, r);
                    }
                }
            }
        }

        let snapshot = runtime.snapshot();
        let projected = projection.project(&snapshot, &usage_reports, &view, timezone);

        if projected.dirty_state {
            let _ = write_atomic(state_dir, "state.json", &projected.state_json);
        }
        if projected.dirty_limits {
            let _ = write_atomic(state_dir, "limits.json", &projected.limits_json);
        }
        let usage_path = state_dir.join("usage.json");
        if should_scan_usage && (projected.dirty_usage || !usage_path.exists()) {
            let _ = write_atomic(state_dir, "usage.json", &projected.usage_json);
        }
        if projected.dirty_view && !view_path.exists() {
            let _ = write_atomic(state_dir, "view.json", &projected.view_json);
        }

        tick = tick.wrapping_add(1);

        // Sleep 1 second in 100ms slices to react promptly to watchdogs
        for _ in 0..10 {
            if stdin_rx.try_recv().is_ok() {
                return Ok(());
            }
            if parent_pid > 1 {
                let ret = unsafe { libc::kill(parent_pid, 0) };
                if ret != 0 {
                    let err = std::io::Error::last_os_error();
                    if err.raw_os_error() == Some(libc::ESRCH) {
                        return Ok(());
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    Ok(())
}

fn run_set_setting(key_str: &str, val_str: &str) -> Result<(), String> {
    let key = match key_str {
        "notificationsMuted" | "notifications-muted" | "notifications_muted" => {
            bar_core::settings::SettingKey::NotificationsMuted
        }
        "showsPreviews" | "shows-previews" | "shows_previews" => {
            bar_core::settings::SettingKey::ShowsPreviews
        }
        other => return Err(format!("Unsupported or unknown setting key: '{other}'")),
    };

    let canonical_key = match key {
        bar_core::settings::SettingKey::NotificationsMuted => "notificationsMuted",
        bar_core::settings::SettingKey::ShowsPreviews => "showsPreviews",
        _ => unreachable!(),
    };

    let val = match val_str.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        other => return Err(format!("Invalid boolean value: '{other}'")),
    };

    let config = Configuration::from_environment(None);
    let settings_file = bar_core::host::settings_path(&config.base_dir);
    let mut store = bar_core::settings::SettingsStore::load(settings_file.clone())
        .unwrap_or_else(|_| bar_core::settings::SettingsStore::defaults(settings_file));

    store.set(key, val).map_err(|e| e.to_string())?;

    let res = serde_json::json!({
        "ok": true,
        "key": canonical_key,
        "value": val,
    });
    println!("{}", serde_json::to_string(&res).map_err(|e| e.to_string())?);
    Ok(())
}

fn run_connect(token: &str) -> Result<(), String> {
    let config = Configuration::from_environment(None);
    let port = config.server_port().map_err(|e| e.to_string())?;
    let fixture = env::var("T3_BAR_FIXTURE_STATE").is_ok();
    let namespace = env::var("T3_BAR_FIXTURE_KEYRING_NAMESPACE").ok();
    let store = bar_core::host::credential_store(fixture, namespace.as_deref())
        .map_err(|e| e.to_string())?;
    let connection = bar_core::limits::LimitsConnection::with_default_transport(store);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;

    rt.block_on(async {
        connection
            .connect(port, token)
            .await
            .map_err(|e| e.to_string())
    })?;

    let res = serde_json::json!({
        "ok": true,
        "status": "connected",
        "port": port,
    });
    println!("{}", serde_json::to_string(&res).map_err(|e| e.to_string())?);
    Ok(())
}

fn run_reconnect() -> Result<(), String> {
    let config = Configuration::from_environment(None);
    let port = config.server_port().map_err(|e| e.to_string())?;
    let fixture = env::var("T3_BAR_FIXTURE_STATE").is_ok();
    let namespace = env::var("T3_BAR_FIXTURE_KEYRING_NAMESPACE").ok();
    let store = bar_core::host::credential_store(fixture, namespace.as_deref())
        .map_err(|e| e.to_string())?;
    let connection = bar_core::limits::LimitsConnection::with_default_transport(store);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;

    rt.block_on(async {
        connection
            .reconnect_saved(port)
            .await
            .map_err(|e| e.to_string())
    })?;

    let res = serde_json::json!({
        "ok": true,
        "status": "reconnected",
        "port": port,
    });
    println!("{}", serde_json::to_string(&res).map_err(|e| e.to_string())?);
    Ok(())
}

fn run_disconnect() -> Result<(), String> {
    let fixture = env::var("T3_BAR_FIXTURE_STATE").is_ok();
    let namespace = env::var("T3_BAR_FIXTURE_KEYRING_NAMESPACE").ok();
    let store = bar_core::host::credential_store(fixture, namespace.as_deref())
        .map_err(|e| e.to_string())?;
    let connection = bar_core::limits::LimitsConnection::with_default_transport(store);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;

    rt.block_on(async {
        connection
            .disconnect()
            .await
            .map_err(|e| e.to_string())
    })?;

    let res = serde_json::json!({
        "ok": true,
        "status": "disconnected",
    });
    println!("{}", serde_json::to_string(&res).map_err(|e| e.to_string())?);
    Ok(())
}

fn open_system_url(url: &str) -> std::io::Result<()> {
    if let Ok(cmd) = env::var("T3_BAR_OPENER_COMMAND") {
        let status = std::process::Command::new(cmd).arg(url).status()?;
        if !status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("opener command exited with {:?}", status.code()),
            ));
        }
        return Ok(());
    }
    if let Ok(cmd) = env::var("T3_BAR_FIXTURE_OPENER") {
        if cmd == "noop" || cmd.trim().is_empty() {
            return Ok(());
        }
        let status = std::process::Command::new(cmd).arg(url).status()?;
        if !status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("opener command exited with {:?}", status.code()),
            ));
        }
        return Ok(());
    }
    if env::var("T3_BAR_FIXTURE_STATE").is_ok() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(not(target_os = "macos"))]
    let cmd = "xdg-open";

    let mut child = std::process::Command::new(cmd).arg(url).spawn()?;
    let status = child.wait()?;
    if !status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("opener command exited with {:?}", status.code()),
        ));
    }
    Ok(())
}

fn run_set_view(state_dir: &Path, args: &[String]) -> Result<(), String> {
    let mut visible: Option<bool> = None;
    let mut screen: Option<String> = None;
    let mut metric: Option<String> = None;
    let mut period: Option<String> = None;

    let mut i = 2;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--visible" {
            visible = Some(true);
        } else if arg == "--hidden" {
            visible = Some(false);
        } else if arg == "--screen" && i + 1 < args.len() {
            i += 1;
            screen = Some(args[i].clone());
        } else if let Some(val) = arg.strip_prefix("--screen=") {
            screen = Some(val.to_string());
        } else if arg == "--metric" && i + 1 < args.len() {
            i += 1;
            metric = Some(args[i].clone());
        } else if let Some(val) = arg.strip_prefix("--metric=") {
            metric = Some(val.to_string());
        } else if arg == "--period" && i + 1 < args.len() {
            i += 1;
            period = Some(args[i].clone());
        } else if let Some(val) = arg.strip_prefix("--period=") {
            period = Some(val.to_string());
        } else if arg == "--state-dir" && i + 1 < args.len() {
            i += 1;
        } else if arg.starts_with("--state-dir=") {
            // skip
        } else {
            return Err(format!("Unknown option: {}", arg));
        }
        i += 1;
    }

    if let Some(ref s) = screen {
        if !["threads", "usage", "settings"].contains(&s.as_str()) {
            return Err(format!("Invalid screen: {}", s));
        }
    }
    if let Some(ref m) = metric {
        if !["cost", "tokens", "limits"].contains(&m.as_str()) {
            return Err(format!("Invalid metric: {}", m));
        }
    }
    if let Some(ref p) = period {
        if !["24h", "7d", "30d", "90d"].contains(&p.as_str()) {
            return Err(format!("Invalid period: {}", p));
        }
    }

    let view_path = state_dir.join("view.json");
    let (cur_vis, cur_scr, cur_met, cur_per, cur_gen) = if view_path.exists() {
        if let Ok(content) = fs::read_to_string(&view_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                (
                    v.get("visible").and_then(|x| x.as_bool()).unwrap_or(false),
                    v.get("screen").and_then(|x| x.as_str()).unwrap_or("threads").to_string(),
                    v.get("metric").and_then(|x| x.as_str()).unwrap_or("cost").to_string(),
                    v.get("period").and_then(|x| x.as_str()).unwrap_or("24h").to_string(),
                    v.get("generation").and_then(|x| x.as_u64()).unwrap_or(0),
                )
            } else {
                (false, "threads".to_string(), "cost".to_string(), "24h".to_string(), 0)
            }
        } else {
            (false, "threads".to_string(), "cost".to_string(), "24h".to_string(), 0)
        }
    } else {
        (false, "threads".to_string(), "cost".to_string(), "24h".to_string(), 0)
    };

    let next_vis = visible.unwrap_or(cur_vis);
    let next_scr = screen.unwrap_or(cur_scr);
    let next_met = metric.unwrap_or(cur_met);
    let next_per = period.unwrap_or(cur_per);
    let next_gen = cur_gen.wrapping_add(1);

    let payload = serde_json::json!({
        "visible": next_vis,
        "screen": next_scr,
        "metric": next_met,
        "period": next_per,
        "generation": next_gen,
    });
    let json_str = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())? + "\n";
    fs::create_dir_all(state_dir).map_err(|e| e.to_string())?;
    write_atomic(state_dir, "view.json", &json_str).map_err(|e| e.to_string())?;

    Ok(())
}

fn run_open_link(state_dir: &Path, args: &[String]) -> Result<(), String> {
    let mut url = None;
    let mut i = 2;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--state-dir" && i + 1 < args.len() {
            i += 2;
            continue;
        } else if arg.starts_with("--state-dir=") {
            i += 1;
            continue;
        } else if !arg.starts_with('-') && url.is_none() {
            url = Some(arg.clone());
        }
        i += 1;
    }

    let url = url.ok_or_else(|| "Usage: t3code-bar-bridge open-link <url>".to_string())?;

    let limits_path = state_dir.join("limits.json");
    let allowed = if limits_path.exists() {
        if let Ok(content) = fs::read_to_string(&limits_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                val.get("links")
                    .and_then(|l| l.as_array())
                    .map(|links| {
                        links.iter().any(|item| {
                            item.get("url").and_then(|u| u.as_str()) == Some(&url)
                        })
                    })
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        let favicons_dir = state_dir.join("favicons");
        let config = Configuration::from_environment(None);
        let timezone = parse_tz();
        let frozen_now = parse_now();
        let runtime = HostRuntime::new(
            config,
            SnapshotMode::HostBridge,
            frozen_now,
            timezone,
            env::var("T3_BAR_FIXTURE_STATE").is_ok(),
            false,
            favicons_dir,
            Arc::new(NoopEventSink),
        );
        runtime.refresh_limits_sync(true);
        let snap = runtime.snapshot();
        snap.limits
            .as_ref()
            .map(|l| {
                l.links.iter().any(|item| {
                    item.get("url").and_then(|u| u.as_str()) == Some(&url)
                })
            })
            .unwrap_or(false)
    };

    if !allowed {
        return Err("Link is not in the current limits snapshot".into());
    }

    open_system_url(&url).map_err(|e| format!("Failed to open link: {}", e))
}

fn parse_cli_state_dir(args: &[String]) -> Option<&str> {
    for i in 1..args.len() {
        if args[i] == "--state-dir" && i + 1 < args.len() {
            return Some(&args[i + 1]);
        }
        if let Some(stripped) = args[i].strip_prefix("--state-dir=") {
            return Some(stripped);
        }
    }
    None
}

fn get_positional_args(args: &[String]) -> Vec<&str> {
    let mut positional = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--state-dir" {
            i += 2;
            continue;
        }
        if args[i].starts_with("--state-dir=") {
            i += 1;
            continue;
        }
        positional.push(args[i].as_str());
        i += 1;
    }
    positional
}

fn get_snapshot_context(
    state_dir: &Path,
) -> Result<(bool, std::collections::HashSet<String>, Configuration), String> {
    let config = Configuration::from_environment(Some(bar_core::host::state_path()));
    let state_file = state_dir.join("state.json");
    if state_file.exists() {
        if let Ok(content) = fs::read_to_string(&state_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if val.get("loading").and_then(|v| v.as_bool()) != Some(true) {
                    let shows_previews = val
                        .get("settings")
                        .and_then(|s| s.get("showsPreviews"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let mut ids = std::collections::HashSet::new();
                    if let Some(arr) = val.get("threads").and_then(|t| t.as_array()) {
                        for item in arr {
                            if let Some(tid) = item.get("id").and_then(|i| i.as_str()) {
                                ids.insert(tid.to_string());
                            }
                        }
                    }
                    return Ok((shows_previews, ids, config));
                }
            }
        }
    }

    let favicons_dir = state_dir.join("favicons");
    let timezone = parse_tz();
    let frozen_now = parse_now();
    let runtime = HostRuntime::new(
        config.clone(),
        SnapshotMode::HostBridge,
        frozen_now,
        timezone,
        env::var("T3_BAR_FIXTURE_STATE").is_ok(),
        false,
        favicons_dir,
        Arc::new(NoopEventSink),
    );
    runtime.refresh_threads_sync();
    let snap = runtime.snapshot();
    let shows_previews = snap.settings.shows_previews;
    let ids = runtime.ids.lock().unwrap().clone();
    Ok((shows_previews, ids, config))
}

fn run_preview(state_dir: &Path, id: &str) -> Result<(), String> {
    let (shows_previews, ids, config) = get_snapshot_context(state_dir)?;
    if !shows_previews {
        println!("null");
        return Ok(());
    }
    if !ids.contains(id) {
        return Err("Unknown thread".to_string());
    }

    if let Ok(delay_str) = env::var("T3_BAR_FIXTURE_PREVIEW_DELAY_MS") {
        if let Ok(ms) = delay_str.parse::<u64>() {
            let delay = ms.min(5000);
            if delay > 0 {
                thread::sleep(Duration::from_millis(delay));
            }
        }
    }

    let preview = bar_core::threads::read_preview(&config, id).map_err(|e| e.to_string())?;
    let json_str = serde_json::to_string(&preview).map_err(|e| e.to_string())?;
    println!("{}", json_str);
    Ok(())
}

fn copy_to_clipboard(text: &str) {
    if env::var("T3_BAR_FIXTURE_OPENER").as_deref() == Ok("noop")
        || env::var("T3_BAR_FIXTURE_STATE").is_ok()
    {
        return;
    }

    if env::var("WAYLAND_DISPLAY").is_ok() {
        if let Ok(mut child) = std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
                let _ = stdin.flush();
                drop(stdin);
            }
            return;
        }
    }
    if let Ok(mut child) = std::process::Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
            let _ = stdin.flush();
            drop(stdin);
        }
    }
}

fn run_thread(state_dir: &Path, id: &str, action: &str) -> Result<(), String> {
    if action != "open" && action != "browser" && action != "copy" {
        return Err("Unknown thread action".to_string());
    }

    let (_shows_previews, ids, config) = get_snapshot_context(state_dir)?;
    if !ids.contains(id) {
        return Err("Unknown thread".to_string());
    }

    let url = config
        .thread_url(id, action == "browser")
        .map_err(|e| e.to_string())?;

    match action {
        "copy" => {
            copy_to_clipboard(&url);
        }
        "open" | "browser" => {
            open_system_url(&url).map_err(|e| e.to_string())?;
        }
        _ => unreachable!(),
    }

    let res = serde_json::json!({
        "ok": true,
        "url": url,
    });
    println!("{}", res);
    Ok(())
}

fn run_new_thread() -> Result<(), String> {
    let url = "t3code://new";
    open_system_url(url).map_err(|e| e.to_string())?;
    let res = serde_json::json!({
        "ok": true,
        "url": url,
    });
    println!("{}", res);
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let positional = get_positional_args(&args);
    if positional.is_empty() {
        eprintln!("Usage: t3code-bar-bridge <subcommand> [options]");
        std::process::exit(1);
    }

    let cli_state_dir = parse_cli_state_dir(&args);
    let state_dir = resolve_state_dir(cli_state_dir);

    match positional[0] {
        "snapshot" => {
            if let Err(e) = run_snapshot(&state_dir) {
                eprintln!("Error running snapshot: {}", e);
                std::process::exit(1);
            }
        }
        "watch" => {
            if let Err(e) = run_watch(&state_dir) {
                eprintln!("Error running watch: {}", e);
                std::process::exit(1);
            }
        }
        "preview" => {
            if positional.len() < 2 {
                eprintln!("Usage: t3code-bar-bridge preview <thread-id>");
                std::process::exit(1);
            }
            if let Err(e) = run_preview(&state_dir, positional[1]) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        "thread" => {
            if positional.len() < 3 {
                eprintln!("Usage: t3code-bar-bridge thread <id> <open|browser|copy>");
                std::process::exit(1);
            }
            if let Err(e) = run_thread(&state_dir, positional[1], positional[2]) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        "new-thread" => {
            if let Err(e) = run_new_thread() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        "set-setting" => {
            if args.len() < 4 {
                eprintln!("Usage: t3code-bar-bridge set-setting <key> <value>");
                std::process::exit(1);
            }
            if let Err(e) = run_set_setting(&args[2], &args[3]) {
                eprintln!("Error setting setting: {}", e);
                std::process::exit(1);
            }
        }
        "connect" => {
            if args.len() < 3 {
                eprintln!("Usage: t3code-bar-bridge connect <token>");
                std::process::exit(1);
            }
            if let Err(e) = run_connect(&args[2]) {
                eprintln!("Error connecting: {}", e);
                std::process::exit(1);
            }
        }
        "reconnect" => {
            if let Err(e) = run_reconnect() {
                eprintln!("Error reconnecting: {}", e);
                std::process::exit(1);
            }
        }
        "disconnect" => {
            if let Err(e) = run_disconnect() {
                eprintln!("Error disconnecting: {}", e);
                std::process::exit(1);
            }
        }
        "set-view" => {
            if let Err(e) = run_set_view(&state_dir, &args) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        "open-link" => {
            if let Err(e) = run_open_link(&state_dir, &args) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        other => {
            eprintln!("Unknown subcommand: {}", other);
            std::process::exit(1);
        }
    }
}
