use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_true")]
    pub supports_vision: bool,
    #[serde(default = "default_auth_header")]
    pub auth_header: String,
    #[serde(default = "default_api_version")]
    pub api_version: String,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_max_count")]
    pub max_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    #[serde(default = "default_ocr_language")]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub ocr: OcrConfig,
}

fn default_provider() -> String {
    "openai".into()
}
fn default_base_url() -> String {
    "https://api.openai.com/v1".into()
}
fn default_model() -> String {
    "gpt-4o".into()
}
fn default_true() -> bool {
    true
}
fn default_auth_header() -> String {
    "Authorization".into()
}
fn default_api_version() -> String {
    "2024-10-21".into()
}
fn default_max_count() -> usize {
    20
}
fn default_ocr_language() -> String {
    "zh-Hans".into()
}
fn default_hotkey() -> String {
    "ctrl+shift+s".into()
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            base_url: default_base_url(),
            api_key: String::new(),
            model: default_model(),
            supports_vision: true,
            auth_header: default_auth_header(),
            api_version: default_api_version(),
            models: Vec::new(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_count: default_max_count(),
        }
    }
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            language: default_ocr_language(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
            hotkey: default_hotkey(),
            cache: CacheConfig::default(),
            ocr: OcrConfig::default(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ai-lens")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn cache_dir() -> PathBuf {
    config_dir().join("cache")
}

/// Returns the loaded config plus an optional human-readable error if the
/// existing config.json could not be read/parsed (in which case defaults are
/// used but the on-disk file is left untouched so the user can fix it).
pub fn load_config() -> (AppConfig, Option<String>) {
    let path = config_path();
    if !path.exists() {
        let config = AppConfig::default();
        save_config(&config).ok();
        return (config, None);
    }
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(config) => (config, None),
            Err(e) => (
                AppConfig::default(),
                Some(format!("config.json 解析失败，已用默认值：{}", e)),
            ),
        },
        Err(e) => (
            AppConfig::default(),
            Some(format!("config.json 读取失败：{}", e)),
        ),
    }
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(config_path(), content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_config(state: tauri::State<'_, Mutex<AppConfig>>) -> AppConfig {
    state.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_config_error(state: tauri::State<'_, Mutex<Option<String>>>) -> Option<String> {
    state.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_config(
    app: AppHandle,
    state: tauri::State<'_, Mutex<AppConfig>>,
    config: AppConfig,
) -> Result<(), String> {
    let old_hotkey = state.lock().unwrap().hotkey.clone();
    save_config(&config)?;
    let new_hotkey = config.hotkey.clone();
    *state.lock().unwrap() = config.clone();
    app.emit("config-changed", &config)
        .map_err(|e| e.to_string())?;

    if old_hotkey != new_hotkey {
        crate::unregister_shortcut(&app, &old_hotkey);
        crate::register_capture_shortcut(&app, &new_hotkey).ok();
    }
    Ok(())
}
