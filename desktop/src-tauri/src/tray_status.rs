#[cfg(not(target_os = "macos"))]
use bar_core::threads::ThreadStatus;
use bar_core::threads::ThreadSummary;

#[cfg(target_os = "macos")]
#[path = "tray_macos.rs"]
mod macos;

pub fn update(app: &tauri::AppHandle, summary: &ThreadSummary) {
    let Some(tray) = app.tray_by_id("bar-tray") else {
        return;
    };
    let status = summary.status;
    let label = format!("{status:?}");
    #[cfg(not(target_os = "macos"))]
    update_icon(&tray, summary);
    #[cfg(target_os = "macos")]
    if let Err(error) = macos::update(&tray, status, summary.badge_count) {
        crate::record("tray_render_error", serde_json::json!({"error":error}));
    }
    if let Err(error) = tray.set_tooltip(Some(format!(
        "T3 Code Bar · {label} · {} running · {} need attention",
        summary.running_count, summary.attention_count
    ))) {
        crate::record(
            "tray_tooltip_error",
            serde_json::json!({"error":error.to_string()}),
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn update_icon(tray: &tauri::tray::TrayIcon, summary: &ThreadSummary) {
    let status = summary.status;
    let label = format!("{status:?}");
    let attention = matches!(
        status,
        ThreadStatus::Approval
            | ThreadStatus::Input
            | ThreadStatus::Request
            | ThreadStatus::Woke
            | ThreadStatus::Failed
            | ThreadStatus::Limited
    );
    let title = if attention {
        if summary.badge_count > 1 {
            format!("{label} {}", summary.badge_count)
        } else {
            label.clone()
        }
    } else if summary.badge_count > 1 {
        summary.badge_count.to_string()
    } else {
        String::new()
    };
    let _ = tray.set_title(Some(&title));
    let image = tauri::include_image!("icons/32x32.png");
    let mut rgba = image.rgba().to_vec();
    if status != ThreadStatus::Settled {
        let color = match status {
            ThreadStatus::Working | ThreadStatus::Goal => [96, 165, 250, 255],
            ThreadStatus::Approval | ThreadStatus::Woke | ThreadStatus::Limited => {
                [251, 191, 36, 255]
            }
            ThreadStatus::Input => [165, 180, 252, 255],
            ThreadStatus::Request => [240, 171, 252, 255],
            ThreadStatus::Failed => [248, 113, 113, 255],
            ThreadStatus::Done => [52, 211, 153, 255],
            _ => [163, 163, 163, 255],
        };
        let width = image.width() as usize;
        let height = image.height() as usize;
        for y in 0..height {
            for x in 0..width {
                let distance =
                    (x as f64 - width as f64 * 0.805).hypot(y as f64 - height as f64 * 0.805);
                let pixel = &mut rgba[(y * width + x) * 4..][..4];
                if distance <= width as f64 * 0.139 {
                    pixel.copy_from_slice(&color);
                } else if distance <= width as f64 * 0.181 {
                    pixel.copy_from_slice(&[0, 0, 0, 255]);
                }
            }
        }
    }
    let _ = tray.set_icon(Some(tauri::image::Image::new_owned(
        rgba,
        image.width(),
        image.height(),
    )));
}
