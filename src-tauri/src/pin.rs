use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

#[derive(Clone, serde::Serialize)]
pub struct PinImage {
    pub base64: String,
    pub width: u32,
    pub height: u32,
}

// Pin images live in memory, keyed by window label, so cache::cleanup can
// never delete the file under a pinned window. Removed on WindowEvent::Destroyed.
#[derive(Default)]
pub struct PinStore {
    pub images: Mutex<HashMap<String, PinImage>>,
    counter: AtomicU32,
}

// MUST be async (a sync command executes inside the WebView2 IPC handler —
// creating a webview there deadlocks the response and every later invoke from
// that window: Esc/right-click looked dead). And the build itself MUST happen
// on the main thread — cross-thread creation hung silently in practice, so we
// hop via run_on_main_thread like run_capture does, with a channel back.
#[tauri::command]
pub async fn pin_image(
    app: AppHandle,
    store: tauri::State<'_, PinStore>,
    base64: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let n = store.counter.fetch_add(1, Ordering::Relaxed) + 1;
    let label = format!("pin-{n}");
    // Insert BEFORE build — the pin webview pulls its image on mount.
    store.images.lock().unwrap().insert(
        label.clone(),
        PinImage {
            base64,
            width,
            height,
        },
    );

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
                    .shadow(true)
                    .visible(false) // the page shows itself once the image is painted
                    .build()?;
            // Builder units are logical; pins are physical. Position first so
            // the DPI assignment from the target monitor lands before the size.
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
        store.images.lock().unwrap().remove(&label);
        return Err(e);
    }
    Ok(label)
}

#[tauri::command]
pub async fn get_pin_image(
    window: tauri::WebviewWindow,
    store: tauri::State<'_, PinStore>,
) -> Result<PinImage, String> {
    store
        .images
        .lock()
        .unwrap()
        .get(window.label())
        .cloned()
        .ok_or_else(|| "图钉数据不存在".into())
}
