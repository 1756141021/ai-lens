use crate::config;
use std::fs;
use std::path::PathBuf;

pub fn ensure_cache_dir() -> Result<PathBuf, String> {
    let dir = config::cache_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn cleanup(max_count: usize) -> Result<(), String> {
    let dir = config::cache_dir();
    if !dir.exists() {
        return Ok(());
    }

    let mut entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("png"))
                .unwrap_or(false)
        })
        // the live overlay frame (Linux frozen-frame capture) is not a crop
        .filter(|e| e.file_name() != *"overlay-frame.png")
        .collect();

    if entries.len() <= max_count {
        return Ok(());
    }

    entries.sort_by_key(|e| {
        e.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    let to_remove = entries.len() - max_count;
    for entry in entries.into_iter().take(to_remove) {
        fs::remove_file(entry.path()).ok();
    }

    Ok(())
}
