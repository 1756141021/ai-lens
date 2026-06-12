// Wayland capture: org.freedesktop.portal.Screenshot is the one screenshot API
// every compositor (Mutter/KWin/wlroots) actually implements. GNOME shows a
// permission dialog on first use and remembers the grant afterwards — IF our
// app-id resolves, which is why the .desktop file matters (see DEV_NOTES).
use std::path::PathBuf;

/// One full-virtual-desktop PNG, written by the portal; caller moves it into
/// the asset-protocol scope and crops the target monitor out of it.
pub async fn grab_frame() -> Result<PathBuf, String> {
    use ashpd::desktop::screenshot::Screenshot;
    let response = Screenshot::request()
        .interactive(false)
        .modal(false)
        .send()
        .await
        .map_err(|e| format!("截屏 portal 请求失败：{e}"))?
        .response()
        .map_err(|e| format!("截屏被拒绝或未完成：{e}"))?;
    response
        .uri()
        .to_file_path()
        .map_err(|_| "截屏 portal 返回的不是本地文件".to_string())
}
