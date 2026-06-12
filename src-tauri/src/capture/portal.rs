// Wayland capture via org.freedesktop.portal.Screenshot (ashpd). Lands in the
// Linux port's stage 2; the stub keeps the session dispatch shape compiling.
use super::CaptureMeta;
use image::RgbaImage;

pub fn grab(_app: &tauri::AppHandle) -> Result<(RgbaImage, CaptureMeta), String> {
    Err("Wayland 截屏后端尚未实现".into())
}
