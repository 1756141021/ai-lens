use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{menu::MenuItem, AppHandle, Manager, Wry};
use tauri_plugin_updater::{Update, UpdaterExt};

const IDLE_TEXT: &str = "检查更新";

pub struct UpdateState {
    // Tray menu item — set_text/set_enabled marshal to the main thread internally.
    pub item: MenuItem<Wry>,
    busy: AtomicBool,
    found: Mutex<Option<Update>>,
}

impl UpdateState {
    pub fn new(item: MenuItem<Wry>) -> Self {
        Self {
            item,
            busy: AtomicBool::new(false),
            found: Mutex::new(None),
        }
    }
}

/// Silent startup check. Offline / 404 (no latest.json yet) / bad JSON all
/// just leave the menu item at "检查更新".
pub fn spawn_startup_check(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return; // dev builds never auto-check
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(updater) = app.updater() else { return };
        if let Ok(Some(update)) = updater.check().await {
            let st = app.state::<UpdateState>();
            st.item.set_text(format!("更新到 v{}", update.version)).ok();
            *st.found.lock().unwrap() = Some(update);
        }
    });
}

pub fn on_menu_click(app: &AppHandle) {
    let st = app.state::<UpdateState>();
    if st.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_flow(&app).await {
            eprintln!("update failed: {e}");
            let st = app.state::<UpdateState>();
            st.item.set_enabled(true).ok();
            st.item.set_text("更新失败，点击重试").ok();
        }
        app.state::<UpdateState>().busy.store(false, Ordering::SeqCst);
    });
}

async fn run_flow(app: &AppHandle) -> Result<(), String> {
    let st = app.state::<UpdateState>();
    // take() under the lock, guard dropped before any await
    let cached = st.found.lock().unwrap().take();
    let update = match cached {
        Some(u) => Some(u),
        None => {
            st.item.set_text("检查中…").ok();
            app.updater()
                .map_err(|e| e.to_string())?
                .check()
                .await
                .map_err(|e| e.to_string())?
        }
    };
    let Some(update) = update else {
        st.item.set_text("已是最新").ok();
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        st.item.set_text(IDLE_TEXT).ok();
        return Ok(());
    };

    st.item.set_enabled(false).ok();
    // install() exits the process — hide the overlay first so a capture in
    // progress doesn't vanish without visual warning.
    if let Some(w) = app.get_webview_window("overlay") {
        w.hide().ok();
    }
    let item = st.item.clone();
    let mut downloaded: u64 = 0;
    let mut last_pct: u64 = u64::MAX;
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                if let Some(total) = total {
                    let pct = downloaded * 100 / total.max(1);
                    if pct != last_pct {
                        last_pct = pct;
                        item.set_text(format!("下载中 {pct}%")).ok();
                    }
                }
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    // On Windows install() spawns the NSIS installer (/UPDATE, passive) and
    // exits this process — the installer relaunches the app. Unreachable here.
    app.restart();
}
