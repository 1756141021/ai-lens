use crate::config::{AppConfig, CursorConfig};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

// Raw Win32 FFI — policy: no `windows` crate dependency (see DEV_NOTES).
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(lp_point: *mut Point) -> i32;
    fn SetWindowPos(
        hwnd: isize,
        hwnd_insert_after: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_NOOWNERZORDER: u32 = 0x0200;

// One process, one ring — module statics keep the follower thread handle-free.
static ENABLED: AtomicBool = AtomicBool::new(false);
static HWND: AtomicIsize = AtomicIsize::new(0);
static DIAMETER: AtomicI32 = AtomicI32::new(32);

pub fn build_cursor_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    let cfg = app
        .state::<Mutex<AppConfig>>()
        .lock()
        .unwrap()
        .cursor
        .clone();
    let d = (cfg.radius.max(2) * 2) as i32;
    let win = WebviewWindowBuilder::new(app, "cursor", WebviewUrl::App("index.html".into()))
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .focusable(false)
        .resizable(false)
        .visible(false)
        .content_protected(true) // WDA_EXCLUDEFROMCAPTURE — the ring never enters screenshots
        .build()
        .map_err(|e| e.to_string())?;
    // Builder sizes are logical px; the ring is sized in physical px (vw/vh fills it).
    win.set_size(PhysicalSize::new(d as u32, d as u32)).ok();
    win.set_ignore_cursor_events(true).ok();
    HWND.store(win.hwnd().map_err(|e| e.to_string())?.0 as isize, Relaxed);
    DIAMETER.store(d, Relaxed);
    if cfg.enabled {
        win.show().ok(); // focusable(false) ⇒ show never steals focus
        ENABLED.store(true, Relaxed);
    }
    Ok(win)
}

/// One detached thread for the app's life; near-idle sleep while disabled.
pub fn spawn_follower() {
    std::thread::spawn(|| {
        let mut last = (i32::MIN, i32::MIN);
        loop {
            if !ENABLED.load(Relaxed) {
                last = (i32::MIN, i32::MIN);
                std::thread::sleep(Duration::from_millis(150));
                continue;
            }
            let hwnd = HWND.load(Relaxed);
            if hwnd != 0 {
                let mut p = Point { x: 0, y: 0 };
                // GetCursorPos fails on the secure desktop (UAC/lock) — skip the tick.
                if unsafe { GetCursorPos(&mut p) } != 0 && (p.x, p.y) != last {
                    last = (p.x, p.y);
                    let d = DIAMETER.load(Relaxed);
                    // cx/cy re-asserted every move (no SWP_NOSIZE): heals the
                    // WM_DPICHANGED resize when crossing mixed-DPI monitors.
                    unsafe {
                        SetWindowPos(
                            hwnd,
                            0,
                            p.x - d / 2,
                            p.y - d / 2,
                            d,
                            d,
                            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
                        );
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(8));
        }
    });
}

/// Reacts to set_config. Color/opacity restyle in the webview via "config-changed".
pub fn apply_config_change(app: &AppHandle, old: &CursorConfig, new: &CursorConfig) {
    let Some(win) = app.get_webview_window("cursor") else {
        return;
    };
    if new.radius != old.radius {
        let d = (new.radius.max(2) * 2) as i32;
        DIAMETER.store(d, Relaxed);
        win.set_size(PhysicalSize::new(d as u32, d as u32)).ok();
    }
    if new.enabled != old.enabled {
        if new.enabled {
            win.show().ok();
            ENABLED.store(true, Relaxed);
        } else {
            ENABLED.store(false, Relaxed);
            win.hide().ok();
        }
    }
}
