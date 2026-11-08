#![allow(deprecated, unexpected_cfgs)]

use block::ConcreteBlock;
use cocoa::{
    appkit::NSWindowCollectionBehavior,
    base::{id, nil, BOOL, YES},
    foundation::{NSPoint, NSRect, NSSize},
};
use objc::{class, msg_send, sel, sel_impl};
use objc::{
    declare::ClassDecl,
    runtime::{Object, Sel},
};
use serde_json::json;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering;
use tauri::Manager;

static PANEL_WINDOW: AtomicPtr<Object> = AtomicPtr::new(nil);

extern "C" {
    fn object_setClass(object: id, class: id) -> id;
}

const NONACTIVATING_PANEL: i32 = 1 << 7;

pub fn prepare_runtime() {}

fn main_thread() -> Result<(), String> {
    let is_main: BOOL = unsafe { msg_send![class!(NSThread), isMainThread] };
    if is_main == YES {
        Ok(())
    } else {
        Err("NSPanel operation requires the main thread".into())
    }
}

fn frontmost_pid() -> i32 {
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let application: id = msg_send![workspace, frontmostApplication];
        msg_send![application, processIdentifier]
    }
}

fn contains(rect: NSRect, point: NSPoint) -> bool {
    point.x >= rect.origin.x
        && point.x <= rect.origin.x + rect.size.width
        && point.y >= rect.origin.y
        && point.y <= rect.origin.y + rect.size.height
}

fn tray_geometry(app: &tauri::AppHandle) -> Result<(NSRect, NSRect), String> {
    let tray = app
        .tray_by_id("bar-tray")
        .ok_or_else(|| "Tray icon is absent".to_string())?;
    tray.with_inner_tray_icon(|tray| unsafe {
        let item = tray
            .ns_status_item()
            .ok_or_else(|| "Status item is absent".to_string())?;
        let item: id = (&*item) as *const _ as id;
        let button: id = msg_send![item, button];
        if button == nil {
            return Err("Status button is absent".to_string());
        }
        let window: id = msg_send![button, window];
        let screen: id = msg_send![window, screen];
        if window == nil || screen == nil {
            return Err("Status button has no window or screen".to_string());
        }
        let bounds: NSRect = msg_send![button, bounds];
        let in_window: NSRect = msg_send![button, convertRect: bounds toView: nil];
        let frame: NSRect = msg_send![window, convertRectToScreen: in_window];
        let visible: NSRect = msg_send![screen, visibleFrame];
        Ok((frame, visible))
    })
    .map_err(|error| error.to_string())?
}

fn tray_contains_point(app: &tauri::AppHandle, point: NSPoint) -> Result<bool, String> {
    tray_geometry(app).map(|(tray, _)| contains(tray, point))
}

fn native_panel() -> Result<id, String> {
    main_thread()?;
    let panel = PANEL_WINDOW.load(Ordering::SeqCst);
    if panel == nil {
        Err("Panel is not initialized".into())
    } else {
        Ok(panel)
    }
}

fn observe(app: &tauri::AppHandle, event: &str) -> Result<(), String> {
    let panel = native_panel()?;
    let window = app
        .get_window("main")
        .ok_or_else(|| "Bar window is absent".to_string())?;
    unsafe {
        let native = if crate::PANEL.load(Ordering::SeqCst) {
            panel
        } else {
            window.ns_window().map_err(|error| error.to_string())? as id
        };
        let frame: NSRect = msg_send![native, frame];
        let active_space: BOOL = msg_send![native, isOnActiveSpace];
        let key: BOOL = msg_send![native, isKeyWindow];
        let visible: BOOL = msg_send![native, isVisible];
        let style: usize = msg_send![native, styleMask];
        let collection: usize = msg_send![native, collectionBehavior];
        let is_panel: BOOL = msg_send![native, isKindOfClass: class!(NSPanel)];
        crate::record(
            event,
            json!({"visible":visible == YES,"nativePanel":is_panel == YES,
            "key":key == YES,"onActiveSpace":active_space == YES,"styleMask":style,
            "collectionBehavior":collection,"frontmostPid":frontmost_pid(),
            "frameNative":[frame.origin.x,frame.origin.y,frame.size.width,frame.size.height]}),
        );
    }
    Ok(())
}

extern "C" fn can_become_key(_: &Object, _: Sel) -> BOOL {
    YES
}

pub fn initialize(app: &mut tauri::App) -> tauri::Result<()> {
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    let window = tauri::window::WindowBuilder::new(app, "main")
        .title("T3 Code Bar")
        .inner_size(320.0, 560.0)
        .visible(false)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .build()?;
    unsafe {
        let mut class = ClassDecl::new("T3BarPanel", class!(NSPanel))
            .ok_or_else(|| std::io::Error::other("Panel class is already registered"))?;
        class.add_method(
            sel!(canBecomeKeyWindow),
            can_become_key as extern "C" fn(&Object, Sel) -> BOOL,
        );
        let class = class.register();
        let panel = window.ns_window()? as id;
        if panel == nil {
            return Err(std::io::Error::other("Native panel window is absent").into());
        }
        // WebKit must attach after conversion so its KVO registrations belong to the final class.
        if object_setClass(panel, class as *const _ as id) == nil {
            return Err(std::io::Error::other("Cannot install native panel class").into());
        }
        PANEL_WINDOW.store(panel, Ordering::SeqCst);
        let _: () = msg_send![panel, setReleasedWhenClosed: cocoa::base::NO];
        let _: () = msg_send![panel, setHidesOnDeactivate: cocoa::base::NO];
        let _: () = msg_send![panel, setFloatingPanel: YES];
        let _: () = msg_send![panel, setStyleMask: NONACTIVATING_PANEL];
        let _: () = msg_send![panel, setLevel: 101i64];
        let _: () = msg_send![panel, setOpaque: cocoa::base::NO];
        let clear: id = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![panel, setBackgroundColor: clear];
        let _: () = msg_send![panel, setHasShadow: YES];
        let content: id = msg_send![panel, contentView];
        let _: () = msg_send![content, setWantsLayer: YES];
        let layer: id = msg_send![content, layer];
        let _: () = msg_send![layer, setCornerRadius: 13.0f64];
        let _: () = msg_send![layer, setMasksToBounds: YES];
        let behavior = NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary;
        let _: () = msg_send![panel, setCollectionBehavior: behavior];
    }
    window.add_child(
        tauri::webview::WebviewBuilder::new("main", tauri::WebviewUrl::App("index.html".into()))
            .auto_resize(),
        tauri::LogicalPosition::new(0.0, 0.0),
        tauri::LogicalSize::new(320.0, 560.0),
    )?;
    let handle = app.handle().clone();
    let handler = ConcreteBlock::new(move |_event: id| {
        if !crate::PANEL.load(Ordering::SeqCst) {
            return;
        }
        match native_panel() {
            Ok(panel) => {
                unsafe {
                    let visible: BOOL = msg_send![panel, isVisible];
                    if visible != YES {
                        return;
                    }
                    let point: NSPoint = msg_send![class!(NSEvent), mouseLocation];
                    match tray_contains_point(&handle, point) {
                        Ok(true) => return,
                        Ok(false) => {}
                        Err(error) => {
                            crate::record("tray_geometry_error", json!({"error":error}));
                        }
                    }
                    let _: () = msg_send![panel, orderOut: nil];
                }
                crate::panel_closed(&handle);
                if let Err(error) = observe(&handle, "outside_click_hidden") {
                    crate::record("panel_observe_error", json!({"error":error}));
                }
            }
            Err(error) => {
                crate::record("panel_error", json!({"error":error}));
            }
        }
    })
    .copy();
    let monitor: id = unsafe {
        msg_send![class!(NSEvent), addGlobalMonitorForEventsMatchingMask: ((1u64 << 1) | (1u64 << 3)) handler: &*handler]
    };
    if monitor == nil {
        return Err(std::io::Error::other("Global mouse monitor unavailable").into());
    }
    let handle = app.handle().clone();
    let resigned = ConcreteBlock::new(move |_notification: id| {
        if !crate::PANEL.load(Ordering::SeqCst) {
            return;
        }
        unsafe {
            let point: NSPoint = msg_send![class!(NSEvent), mouseLocation];
            let application: id = msg_send![class!(NSApplication), sharedApplication];
            let event: id = msg_send![application, currentEvent];
            let kind: usize = if event == nil {
                0
            } else {
                msg_send![event, type]
            };
            if matches!(kind, 1..=4) {
                match tray_contains_point(&handle, point) {
                    Ok(true) => return,
                    Ok(false) => {}
                    Err(error) => {
                        crate::record("tray_geometry_error", json!({"error":error}));
                    }
                }
            }
            match native_panel() {
                Ok(panel) => {
                    let _: () = msg_send![panel, orderOut: nil];
                }
                Err(error) => {
                    crate::record("panel_error", json!({"error":error}));
                }
            }
        }
        crate::panel_closed(&handle);
        if let Err(error) = observe(&handle, "resigned_key_hidden") {
            crate::record("panel_observe_error", json!({"error":error}));
        }
    })
    .copy();
    unsafe {
        let center: id = msg_send![class!(NSNotificationCenter), defaultCenter];
        let name: id = msg_send![class!(NSString), stringWithUTF8String: c"NSWindowDidResignKeyNotification".as_ptr()];
        if center == nil || name == nil {
            return Err(std::io::Error::other("Cannot access notification center").into());
        }
        let panel = native_panel().map_err(std::io::Error::other)?;
        let token: id = msg_send![center, addObserverForName: name object: panel queue: nil usingBlock: &*resigned];
        if token == nil {
            return Err(std::io::Error::other("Cannot observe panel focus").into());
        }
    }
    observe(app.handle(), "panel_initialized")
        .map_err(|error| std::io::Error::other(format!("Cannot record panel state: {error}")))?;
    Ok(())
}

pub fn hide(window: &tauri::Window) -> Result<(), String> {
    let panel = native_panel()?;
    unsafe {
        let native = if crate::PANEL.load(Ordering::SeqCst) {
            panel
        } else {
            window.ns_window().map_err(|e| e.to_string())? as id
        };
        let _: () = msg_send![native, orderOut: nil];
    }
    crate::panel_closed(window.app_handle());
    observe(window.app_handle(), "hidden")?;
    Ok(())
}

pub fn show(app: &tauri::AppHandle) -> Result<(), String> {
    let panel = native_panel()?;
    let window = app.get_window("main").ok_or("Bar window is absent")?;
    let before = frontmost_pid();
    if crate::PANEL.load(Ordering::SeqCst) {
        let (tray, screen) = tray_geometry(app)?;
        let height = crate::PANEL_HEIGHT.load(Ordering::Acquire) as f64;
        let frame = anchored_frame(tray, screen, 320.0, height);
        unsafe {
            let _: () = msg_send![panel, setFrame: frame display: YES];
            let _: () = msg_send![panel, makeKeyAndOrderFront: nil];
        }
        crate::record(
            "anchor",
            json!({"trayNative":[tray.origin.x,tray.origin.y,tray.size.width,tray.size.height],
            "visibleScreenNative":[screen.origin.x,screen.origin.y,screen.size.width,screen.size.height]}),
        );
    } else {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    crate::record(
        "activation",
        json!({"frontmostBefore":before,"frontmostAfter":frontmost_pid()}),
    );
    crate::panel_opened(app);
    observe(app, "shown")?;
    Ok(())
}

pub fn toggle(app: &tauri::AppHandle) {
    let result = (|| {
        let window = app.get_window("main").ok_or("Bar window is absent")?;
        let panel = native_panel()?;
        let native = if crate::PANEL.load(Ordering::SeqCst) {
            panel
        } else {
            window.ns_window().map_err(|e| e.to_string())? as id
        };
        let visible: BOOL = unsafe { msg_send![native, isVisible] };
        if visible == YES {
            hide(&window)
        } else {
            show(app)
        }
    })();
    if let Err(error) = result {
        crate::record("panel_error", json!({"error":error}));
    }
}

pub fn on_window_event(_window: &tauri::Window, _event: &tauri::WindowEvent) {}

pub fn resize(_window: &tauri::Window, height: u32) -> Result<(), String> {
    let panel = native_panel()?;
    unsafe {
        let mut frame: NSRect = msg_send![panel, frame];
        frame.origin.y += frame.size.height - height as f64;
        frame.size = NSSize::new(320.0, height as f64);
        let _: () = msg_send![panel, setFrame: frame display: YES];
    }
    Ok(())
}

fn anchored_frame(
    tray: NSRect,
    screen: NSRect,
    requested_width: f64,
    requested_height: f64,
) -> NSRect {
    let width = requested_width.min((screen.size.width - 16.0).max(1.0));
    let height = requested_height.min((screen.size.height - 16.0).max(1.0));
    let x = (tray.origin.x + tray.size.width / 2.0 - width / 2.0).clamp(
        screen.origin.x + 8.0,
        screen.origin.x + screen.size.width - width - 8.0,
    );
    let y = (tray.origin.y - height - 4.0).clamp(
        screen.origin.y + 8.0,
        screen.origin.y + screen.size.height - height - 8.0,
    );
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchored_frame_stays_under_tray() {
        let screen = NSRect::new(NSPoint::new(100.0, 50.0), NSSize::new(1200.0, 800.0));
        let tray = NSRect::new(NSPoint::new(620.0, 820.0), NSSize::new(24.0, 22.0));
        let frame = anchored_frame(tray, screen, 320.0, 560.0);
        assert!((frame.size.width - 320.0).abs() < f64::EPSILON);
        assert!((frame.size.height - 560.0).abs() < f64::EPSILON);
        assert!((frame.origin.x - 472.0).abs() < f64::EPSILON);
        assert!((frame.origin.y - 256.0).abs() < f64::EPSILON);
    }

    #[test]
    fn anchored_frame_clamps_oversized_panel_to_screen() {
        let screen = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 300.0));
        let tray = NSRect::new(NSPoint::new(-200.0, 300.0), NSSize::new(20.0, 20.0));
        let frame = anchored_frame(tray, screen, 800.0, 600.0);
        assert!((frame.size.width - 384.0).abs() < f64::EPSILON);
        assert!((frame.size.height - 284.0).abs() < f64::EPSILON);
        assert!((frame.origin.x - 8.0).abs() < f64::EPSILON);
        assert!((frame.origin.y - 8.0).abs() < f64::EPSILON);
    }
}
