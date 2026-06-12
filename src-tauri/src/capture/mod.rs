pub(crate) mod screen;

#[cfg(target_os = "linux")]
pub(crate) mod portal;

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
    /// Linux: the overlay paints this frame instead of sitting transparent over
    /// the live desktop (Wayland forbids positioning + transparency is flaky).
    pub frozen_frame: bool,
    /// PNG to paint (served via the asset protocol); None on Windows.
    pub frame_path: Option<String>,
    /// Where this monitor's rect sits inside the frame file (Wayland portal
    /// shots cover the whole virtual desktop; X11/xcap shots are already
    /// monitor-sized so the offset is 0).
    pub frame_offset_x: u32,
    pub frame_offset_y: u32,
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

#[cfg(target_os = "linux")]
pub fn wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|v| v.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
        || std::env::var("WAYLAND_DISPLAY").is_ok()
}

/// Stash the grabbed frame + geometry for the overlay (pull) and the later
/// region crop. Shared by every backend.
pub fn store(last: &LastCapture, meta: &CaptureMetaState, img: RgbaImage, m: CaptureMeta) {
    *last.lock().unwrap() = Some(img);
    *meta.lock().unwrap() = Some(m);
}

/// Windows: capture the monitor under the cursor into memory (for later
/// cropping) and store its geometry. No encode happens — the overlay is
/// transparent and shows the live desktop, so there's no frozen frame to ship.
/// (Linux capture is orchestrated async in lib.rs — the Wayland portal call
/// can block on a permission dialog.)
#[cfg(windows)]
pub fn grab(
    app: &tauri::AppHandle,
    last: &LastCapture,
    meta: &CaptureMetaState,
) -> Result<CaptureMeta, String> {
    let g = screen::grab(app)?;
    let m = CaptureMeta {
        width: g.img.width(),
        height: g.img.height(),
        origin_x: g.origin_x,
        origin_y: g.origin_y,
        scale: g.scale,
        frozen_frame: false,
        frame_path: None,
        frame_offset_x: 0,
        frame_offset_y: 0,
    };
    store(last, meta, g.img, m.clone());
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
