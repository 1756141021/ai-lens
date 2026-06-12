use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

/// The first turn a chat window opens with, parked here until its webview
/// mounts and pulls it (same pattern as pin.rs: insert BEFORE building).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SeedTurn {
    pub text: String,
    pub image: Option<String>,
    pub ocr: Option<String>,
}

#[derive(Clone, serde::Serialize)]
pub struct ChatInfo {
    pub label: String,
    pub title: String,
}

#[derive(Default)]
pub struct ChatStore {
    pub seeds: Mutex<HashMap<String, SeedTurn>>,
    pub titles: Mutex<HashMap<String, String>>,
    counter: AtomicU32,
}

// Window creation rules are the hard-won ones from pin.rs: MUST be an async
// command (sync runs inside the WebView2 IPC handler → deadlock), and the
// build MUST hop to the main thread via run_on_main_thread + channel.
#[tauri::command]
pub async fn spawn_chat(
    app: AppHandle,
    store: tauri::State<'_, ChatStore>,
    turn: SeedTurn,
    title: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let n = store.counter.fetch_add(1, Ordering::Relaxed) + 1;
    let label = format!("chat-{n}");
    store.seeds.lock().unwrap().insert(label.clone(), turn);
    store.titles.lock().unwrap().insert(label.clone(), title);

    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let handle = app.clone();
    let win_label = label.clone();
    app.run_on_main_thread(move || {
        let built = (|| -> tauri::Result<()> {
            let win =
                WebviewWindowBuilder::new(&handle, &win_label, WebviewUrl::App("index.html".into()))
                    .decorations(false)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .maximizable(false) // drag-region double-click would otherwise toggle-maximize
                    .resizable(false)
                    .transparent(true) // rounded glass panel over the live desktop
                    .shadow(false)
                    .visible(false) // the page shows itself once the first frame is ready
                    .build()?;
            // Builder units are logical; we position in physical. Position first
            // so the target monitor's DPI lands before the size.
            win.set_position(PhysicalPosition::new(x, y))?;
            win.set_size(PhysicalSize::new(width, height))?;
            Ok(())
        })();
        tx.send(built.map_err(|e| e.to_string())).ok();
    })
    .map_err(|e| e.to_string())?;

    let built = tokio::task::spawn_blocking(move || {
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .unwrap_or_else(|e| Err(e.to_string()))
    })
    .await
    .map_err(|e| e.to_string())?;

    if let Err(e) = built {
        store.seeds.lock().unwrap().remove(&label);
        store.titles.lock().unwrap().remove(&label);
        return Err(e);
    }
    Ok(label)
}

#[tauri::command]
pub async fn get_chat_seed(
    window: tauri::WebviewWindow,
    store: tauri::State<'_, ChatStore>,
) -> Result<SeedTurn, String> {
    store
        .seeds
        .lock()
        .unwrap()
        .get(window.label())
        .cloned()
        .ok_or_else(|| "对话数据不存在".into())
}

/// Open conversations, for the 追加 target picker.
#[tauri::command]
pub fn list_chats(store: tauri::State<'_, ChatStore>) -> Vec<ChatInfo> {
    let titles = store.titles.lock().unwrap();
    let mut out: Vec<ChatInfo> = titles
        .iter()
        .map(|(label, title)| ChatInfo {
            label: label.clone(),
            title: title.clone(),
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// Hand a freshly framed capture to an existing chat window: it stages the
/// image for its next follow-up and comes to the front.
#[tauri::command]
pub fn append_to_chat(
    app: AppHandle,
    label: String,
    image: String,
    ocr: Option<String>,
) -> Result<(), String> {
    let win = app
        .get_webview_window(&label)
        .ok_or_else(|| "对话窗口已关闭".to_string())?;
    win.emit("stage-image", serde_json::json!({ "image": image, "ocr": ocr }))
        .map_err(|e| e.to_string())?;
    win.show().ok();
    win.set_focus().ok();
    Ok(())
}

pub fn on_destroyed(window: &tauri::Window) {
    if let Some(store) = window.app_handle().try_state::<ChatStore>() {
        store.seeds.lock().unwrap().remove(window.label());
        store.titles.lock().unwrap().remove(window.label());
    }
}
