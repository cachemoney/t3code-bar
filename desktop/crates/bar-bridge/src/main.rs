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

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: t3code-bar-bridge <subcommand> [options]");
        std::process::exit(1);
    }

    let cli_state_dir = parse_cli_state_dir(&args);
    let state_dir = resolve_state_dir(cli_state_dir);

    match args[1].as_str() {
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
