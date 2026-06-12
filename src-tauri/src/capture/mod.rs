mod screen;

#[cfg(target_os = "linux")]
mod portal;

use crate::cache;
use crate::config::AppConfig;
use base64::{engine::general_purpose::STANDARD, Engine};
use image::RgbaImage;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// The last full-screen capture, kept in memory so region cropping doesn't pay
/// for a full-screen PNG write+read round-trip on the capture hot path.
pub type LastCapture = Mutex<Option<RgbaImage>>;
/// The metadata of the last capture, served to the overlay window.
pub type CaptureMetaState = Mutex<Option<CaptureMeta>>;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureMeta {
    pub width: u32, // physical px
    pub height: u32,
    pub origin_x: i32, // monitor origin in the virtual desktop (physical px)
    pub origin_y: i32,
    pub scale: f64, // monitor scale factor (DPI)
}

#[derive(serde::Serialize)]
pub struct CaptureResult {
    pub path: String,
    pub base64: String,
    pub width: u32,
    pub height: u32,
}

fn encode_png(img: &image::RgbaImage) -> Result<String, String> {
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(STANDARD.encode(&buf))
}

/// Session-appropriate backend: xcap everywhere except Wayland, which only
/// hands out pixels through the desktop portal.
fn backend_grab(app: &tauri::AppHandle) -> Result<(RgbaImage, CaptureMeta), String> {
    #[cfg(target_os = "linux")]
    if wayland_session() {
        return portal::grab(app);
    }
    screen::grab(app)
}

#[cfg(target_os = "linux")]
fn wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|v| v.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
        || std::env::var("WAYLAND_DISPLAY").is_ok()
}

/// Capture the monitor under the cursor into memory (for later cropping) and
/// store its geometry. No encode happens — the overlay is transparent and shows
/// the live desktop, so there's no frozen frame to ship. Called on hotkey/tray.
pub fn grab(
    app: &tauri::AppHandle,
    last: &LastCapture,
    meta: &CaptureMetaState,
) -> Result<CaptureMeta, String> {
    let (img, m) = backend_grab(app)?;
    *last.lock().unwrap() = Some(img);
    *meta.lock().unwrap() = Some(m.clone());
    Ok(m)
}

/// The overlay window pulls the frozen frame + metadata (no IPC race vs push).
#[tauri::command]
pub fn get_capture_meta(meta: tauri::State<'_, CaptureMetaState>) -> Option<CaptureMeta> {
    meta.lock().unwrap().clone()
}

#[tauri::command]
pub fn capture_region(
    state: tauri::State<'_, Mutex<AppConfig>>,
    last: tauri::State<'_, LastCapture>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<CaptureResult, String> {
    let cropped = {
        let guard = last.lock().unwrap();
        let full = guard.as_ref().ok_or("没有可用的截图")?;
        let x = x.min(full.width().saturating_sub(1));
        let y = y.min(full.height().saturating_sub(1));
        let cw = width.min(full.width().saturating_sub(x));
        let ch = height.min(full.height().saturating_sub(y));
        if cw == 0 || ch == 0 {
            return Err("选区超出截图范围".into());
        }
        image::imageops::crop_imm(full, x, y, cw, ch).to_image()
    };

    let cache_dir = cache::ensure_cache_dir()?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let filepath = cache_dir.join(format!("{}.png", ts));
    cropped.save(&filepath).map_err(|e| e.to_string())?;

    let b64 = encode_png(&cropped)?;

    let config = state.lock().unwrap();
    cache::cleanup(config.cache.max_count).ok();

    Ok(CaptureResult {
        path: filepath.to_string_lossy().into(),
        base64: b64,
        width: cropped.width(),
        height: cropped.height(),
    })
}
