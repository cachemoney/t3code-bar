use gtk::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

static HOST: AtomicBool = AtomicBool::new(false);
static TRAY: AtomicBool = AtomicBool::new(false);

// Must run before GTK/WebKit is initialized. Respect even an explicit empty override.
pub fn prepare_runtime() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

fn host_registered() -> bool {
    let result = (|| -> zbus::Result<bool> {
        let connection = zbus::blocking::Connection::session()?;
        let proxy = zbus::blocking::Proxy::new(
            &connection,
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
        )?;
        if !proxy.get_property::<bool>("IsStatusNotifierHostRegistered")? {
            return Ok(false);
        }
        let items: Vec<String> =
            proxy.get_property::<Vec<String>>("RegisteredStatusNotifierItems")?;
        let bus = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        )?;
        Ok(items.iter().any(|item| {
            let service = item.split('/').next().unwrap_or("");
            bus.call::<_, _, u32>("GetConnectionUnixProcessID", &(service,))
                .map(|pid| pid == std::process::id())
                .unwrap_or(false)
        }))
    })();
    result.unwrap_or(false)
}

fn available() -> bool {
    TRAY.load(Ordering::SeqCst) && HOST.load(Ordering::SeqCst)
}

fn apply(window: &tauri::Window, panel: bool) -> Result<(), String> {
    let panel_effective = panel && available();
    let height = crate::PANEL_HEIGHT.load(Ordering::Acquire) as i32;
    let gtk = window.gtk_window().map_err(|e| e.to_string())?;
    gtk.set_size_request(
        if panel_effective { 320 } else { -1 },
        if panel_effective { height } else { -1 },
    );
    gtk.set_default_size(320, height);
    gtk.set_decorated(!panel_effective);
    // The optional Hyprbars rule matches this title only while the panel is effective.
    gtk.set_title(if panel_effective {
        "T3 Code Bar — Panel"
    } else {
        "T3 Code Bar"
    });
    let gtk_observed = gtk.clone();
    gtk::glib::timeout_add_local_once(Duration::from_millis(1000), move || {
        crate::record(
            "linux_gtk_state",
            serde_json::json!({"decorated":gtk_observed.is_decorated(),"mapped":gtk_observed.is_mapped(),"realized":gtk_observed.is_realized(),"titlebar":gtk_observed.titlebar().is_some()}),
        );
    });
    window
        .set_skip_taskbar(panel_effective)
        .map_err(|e| e.to_string())?;
    window
        .set_always_on_top(panel_effective)
        .map_err(|e| e.to_string())?;
    window.set_resizable(true).map_err(|e| e.to_string())?;
    window
        .set_min_size(if panel_effective {
            Some(tauri::LogicalSize::new(320.0, height as f64))
        } else {
            None
        })
        .map_err(|e| e.to_string())?;
    window
        .set_max_size(if panel_effective {
            Some(tauri::LogicalSize::new(320.0, height as f64))
        } else {
            None
        })
        .map_err(|e| e.to_string())?;
    window
        .set_size(tauri::LogicalSize::new(320.0, height as f64))
        .map_err(|e| e.to_string())?;
    let size = window.inner_size().ok();
    crate::record(
        "linux_variant",
        serde_json::json!({"requestedPanel":panel,"effectivePanel":panel_effective,"host":HOST.load(Ordering::SeqCst),"tray":TRAY.load(Ordering::SeqCst),"requestedLogicalSize":[320,560],"actualPhysicalSize":size.map(|s| [s.width,s.height]),"scaleFactor":window.scale_factor().ok()}),
    );
    Ok(())
}

pub fn initialize(app: &mut tauri::App) -> tauri::Result<()> {
    let host = host_registered();
    HOST.store(host, Ordering::SeqCst);
    let tray_result = (|| -> tauri::Result<_> {
        let show_item = MenuItem::with_id(app, "show", "Show T3 Code Bar", true, None::<&str>)?;
        let hide_item = MenuItem::with_id(app, "hide", "Hide T3 Code Bar", true, None::<&str>)?;
        let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit T3 Code Bar", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&show_item, &hide_item, &settings, &quit])?;
        let mut rgba = vec![0u8; 16 * 16 * 4];
        for pixel in rgba.chunks_mut(4) {
            pixel.copy_from_slice(&[96, 165, 250, 255]);
        }
        TrayIconBuilder::with_id("bar-tray").icon(tauri::image::Image::new_owned(rgba,16,16))
            .tooltip("T3 Code Bar").menu(&menu)
            .on_menu_event(|app, event| {
                let id = event.id.as_ref();
                let window = app.get_window("main");
                let before = window.as_ref().and_then(|w| w.is_visible().ok());
                let result: Result<(), String> = match (id, window.as_ref()) {
                    ("show", Some(_)) => show(app),
                    ("settings", Some(_)) => show(app).and_then(|_| app.emit("bar-settings", ()).map_err(|e| e.to_string())),
                    ("hide", Some(w)) => hide(w),
                    ("quit", _) => Ok(()),
                    (_, None) => Err("Bar window missing".into()),
                    _ => Err("Unknown menu action".into()),
                };
                let after = window.as_ref().and_then(|w| w.is_visible().ok());
                crate::record("tray_menu", serde_json::json!({"id":id,"visibleBefore":before,"visibleAfter":after,"error":result.as_ref().err()}));
                if let Err(error) = result { eprintln!("Linux tray action {id}: {error}"); }
                if let Some(window) = window {
                    let action = id.to_owned();
                    gtk::glib::timeout_add_local_once(Duration::from_millis(250), move || {
                        let visible = window.is_visible().map_err(|e| e.to_string());
                        crate::record("tray_menu_settled", serde_json::json!({"id":action,"visible":visible.as_ref().ok(),"error":visible.as_ref().err()}));
                        if let Err(error) = visible { eprintln!("Linux tray visibility: {error}"); }
                    });
                }
                if id == "quit" { app.exit(0); }
            })
            .build(app)
    })();
    TRAY.store(tray_result.is_ok(), Ordering::SeqCst);
    crate::record(
        "linux_tray_init",
        serde_json::json!({"host":host,"trayCreated":tray_result.is_ok(),"error":tray_result.err().map(|e|e.to_string())}),
    );
    let app = app.handle().clone();
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut initial = true;
        loop {
            let host = host_registered();
            let changed = HOST.swap(host, Ordering::SeqCst) != host;
            if changed || (initial && (host || start.elapsed() >= Duration::from_secs(2))) {
                let first = initial;
                initial = false;
                crate::record(
                    "linux_host_changed",
                    serde_json::json!({"host":host,"initial":first}),
                );
                let handle = app.clone();
                if let Err(error) = app.run_on_main_thread(move || {
                    if let Some(window) = handle.get_window("main") {
                        if let Err(error) = apply(&window, crate::PANEL.load(Ordering::SeqCst)) { eprintln!("Linux window setup: {error}"); crate::record("linux_error", serde_json::json!({"operation":"apply","error":error})); }
                        if first || !available() {
                            if let Err(error) = show(&handle) { eprintln!("Linux window show: {error}"); crate::record("linux_error", serde_json::json!({"operation":"show","error":error.to_string()})); }
                        }
                    }
                }) { eprintln!("Linux main thread: {error}"); }
            }
            std::thread::sleep(Duration::from_millis(if initial { 100 } else { 500 }));
        }
    });
    Ok(())
}

pub fn set_variant(window: &tauri::Window, panel: bool) -> Result<(), String> {
    apply(window, panel)
}

pub fn hide(window: &tauri::Window) -> Result<(), String> {
    if !available() {
        crate::record(
            "linux_hide_refused",
            serde_json::json!({"reason":"no tray host"}),
        );
        return Err("No reachable tray host; window remains visible".into());
    }
    window.hide().map_err(|e| e.to_string())?;
    crate::panel_closed(window.app_handle());
    Ok(())
}

pub fn toggle(app: &tauri::AppHandle) {
    if let Some(window) = app.get_window("main") {
        if window.is_visible().unwrap_or(true) {
            let _ = hide(&window);
        } else {
            let _ = show(app);
        }
    }
}

pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::Focused(focused) => {
            crate::record(
                "linux_focus",
                serde_json::json!({"focused":focused,"host":available()}),
            );
            if !focused && crate::PANEL.load(Ordering::SeqCst) && available() {
                if let Some(webview) = window.app_handle().get_window(window.label()) {
                    if let Err(error) = hide(&webview) {
                        eprintln!("Linux focus hide: {error}");
                        crate::record(
                            "linux_error",
                            serde_json::json!({"operation":"focus_hide","error":error}),
                        );
                    }
                }
            }
        }
        tauri::WindowEvent::Resized(size) => crate::record(
            "linux_resize",
            serde_json::json!({"actualPhysicalSize":[size.width,size.height],"requestedLogicalSize":[320,560]}),
        ),
        _ => {}
    }
}

pub fn show(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app.get_window("main").ok_or("Bar window missing")?;
    window
        .show()
        .and_then(|_| window.set_focus())
        .map_err(|e| e.to_string())?;
    crate::panel_opened(app);
    Ok(())
}
pub fn resize(window: &tauri::Window, _height: u32) -> Result<(), String> {
    apply(window, crate::PANEL.load(Ordering::Acquire))
}
