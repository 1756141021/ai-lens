// xcap-backed grab: Windows always, Linux only on X11 sessions. Wayland goes
// through portal.rs — xcap's Wayland paths lean on a private GNOME API
// (rejected since GNOME 41) and wlr-screencopy (absent on Mutter/KWin).
use super::CaptureMeta;
use image::RgbaImage;
use xcap::Monitor;

fn monitor_under_cursor(app: &tauri::AppHandle) -> Result<Monitor, String> {
    if let Ok(pos) = app.cursor_position() {
        if let Ok(m) = Monitor::from_point(pos.x as i32, pos.y as i32) {
            return Ok(m);
        }
    }
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    monitors
        .into_iter()
        .next()
        .ok_or_else(|| "No monitor found".to_string())
}

pub fn grab(app: &tauri::AppHandle) -> Result<(RgbaImage, CaptureMeta), String> {
    let monitor = monitor_under_cursor(app)?;
    let origin_x = monitor.x().map_err(|e| e.to_string())?;
    let origin_y = monitor.y().map_err(|e| e.to_string())?;
    let scale = monitor.scale_factor().map_err(|e| e.to_string())? as f64;

    let img = monitor.capture_image().map_err(|e| e.to_string())?;
    let meta = CaptureMeta {
        width: img.width(),
        height: img.height(),
        origin_x,
        origin_y,
        scale,
    };
    Ok((img, meta))
}
