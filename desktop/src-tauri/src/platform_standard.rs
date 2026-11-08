use std::sync::atomic::Ordering;
use tauri::Manager;

pub fn prepare_runtime() {}

pub fn initialize(_app: &mut tauri::App) -> tauri::Result<()> { Ok(()) }

pub fn set_variant(window: &tauri::Window, panel: bool) -> Result<(), String> {
    window.set_decorations(!panel).map_err(|e| e.to_string())?;
    window.set_always_on_top(panel).map_err(|e| e.to_string())
}

pub fn hide(window: &tauri::Window) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

pub fn toggle(app: &tauri::AppHandle) {
    if let Some(window) = app.get_window("main") {
        if window.is_visible().unwrap_or(false) { let _ = hide(&window); }
        else { let _ = window.show(); let _ = window.set_focus(); }
    }
}

pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if matches!(event, tauri::WindowEvent::Focused(false)) && crate::PANEL.load(Ordering::SeqCst) { let _ = window.hide(); }
}
