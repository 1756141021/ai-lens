mod cache;
mod capture;
mod config;
mod cursor;
mod ocr;
mod updater;

use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const DEFAULT_HOTKEY: &str = "ctrl+shift+s";

/// Register the capture global shortcut. On press it runs the capture on the
/// main thread (window creation must happen there).
pub fn register_capture_shortcut(app: &AppHandle, spec: &str) -> Result<(), String> {
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(spec, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let h = handle.clone();
                handle
                    .run_on_main_thread(move || {
                        let _ = run_capture(&h);
                    })
                    .ok();
            }
        })
        .map_err(|e| e.to_string())
}

pub fn unregister_shortcut(app: &AppHandle, spec: &str) {
    app.global_shortcut().unregister(spec).ok();
}

/// Build the fullscreen, opaque, borderless, always-on-top overlay window
/// (hidden). Prebuilt once at startup and reused for every capture so the hot
/// path never pays for a webview rebuild.
fn build_overlay(app: &AppHandle) -> Result<WebviewWindow, String> {
    WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html".into()))
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .transparent(true) // QQ-style: dim layer over the live desktop, no frozen frame
        .focused(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())
}

/// Capture the monitor under the cursor, position the (reused) overlay over it,
/// and signal the frontend. The frontend shows the window itself once it has
/// painted the frozen frame — no rebuild, no stale/black flash on reuse.
fn run_capture(app: &AppHandle) -> Result<(), String> {
    let last = app.state::<capture::LastCapture>();
    let meta = app.state::<capture::CaptureMetaState>();
    let m = capture::grab(app, &last, &meta)?;

    let win = match app.get_webview_window("overlay") {
        Some(w) => w,
        None => build_overlay(app)?,
    };

    win.set_position(PhysicalPosition::new(m.origin_x, m.origin_y))
        .map_err(|e| e.to_string())?;
    win.set_size(PhysicalSize::new(m.width, m.height))
        .map_err(|e| e.to_string())?;
    // Reused overlay: its listener re-inits + shows. Freshly built overlay (only
    // if prebuild failed): its onMount reads the meta and shows itself.
    app.emit("capture-ready", ()).ok();
    Ok(())
}

fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        w.show().ok();
        w.set_focus().ok();
        return;
    }
    if let Ok(w) = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title("AI Lens 设置")
        .inner_size(420.0, 560.0)
        .resizable(false)
        .center()
        .build()
    {
        w.show().ok();
        w.set_focus().ok();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (app_config, config_error) = config::load_config();
    let hotkey_spec = app_config.hotkey.clone();
    let needs_setup = app_config.api.api_key.trim().is_empty();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Mutex::new(app_config))
        .manage(Mutex::new(config_error))
        .manage(Mutex::new(None::<image::RgbaImage>))
        .manage(Mutex::new(None::<capture::CaptureMeta>))
        .setup(move |app| {
            let cap = MenuItem::with_id(app, "capture", "截图", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let upd = MenuItem::with_id(app, "update", "检查更新", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&cap, &settings, &upd, &quit])?;
            app.manage(updater::UpdateState::new(upd.clone()));

            let icon = app
                .default_window_icon()
                .cloned()
                .unwrap_or_else(|| tauri::image::Image::new(&[0u8; 4], 1, 1));

            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("AI Lens")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "settings" => open_settings(app),
                    "update" => updater::on_menu_click(app),
                    "capture" => {
                        let _ = run_capture(app);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = run_capture(tray.app_handle());
                    }
                })
                .build(app)?;

            let handle = app.handle().clone();
            if register_capture_shortcut(&handle, &hotkey_spec).is_err() {
                register_capture_shortcut(&handle, DEFAULT_HOTKEY).ok();
            }

            // Prewarm the overlay (hidden) so the first capture is as fast as the rest.
            build_overlay(&handle).ok();

            cursor::build_cursor_window(&handle).ok();
            cursor::spawn_follower();

            updater::spawn_startup_check(&handle);

            // First run (no API key yet): open Settings so the user can configure.
            if needs_setup {
                open_settings(&handle);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Settings closes to a hidden, reusable window; overlay closes for real.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "settings" {
                    api.prevent_close();
                    window.hide().ok();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            capture::get_capture_meta,
            capture::capture_region,
            config::get_config,
            config::get_config_error,
            config::set_config,
            ocr::ocr_image,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Tray-only app: don't exit when the last window closes.
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}
