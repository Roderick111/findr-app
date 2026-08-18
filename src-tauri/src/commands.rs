use crate::background::{IndexActivity, IndexActivityState, SyncLock};
use crate::findr_client::{self, DoctorReport, IndexStatus, SearchResponse};
use crate::license::{self, LicenseState, LicenseStatus, ValidationCacheState};
use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::AutoLaunchManager;
use tauri_plugin_store::StoreExt;

const MAX_AUTHORIZED_PATHS: usize = 2_048;
const MAX_PREVIEW_BYTES: u64 = 50_000;

#[derive(Default)]
struct AuthorizedPathSet {
    paths: HashSet<PathBuf>,
    order: VecDeque<PathBuf>,
}

#[derive(Default)]
pub struct AuthorizedPaths(Mutex<AuthorizedPathSet>);

impl AuthorizedPaths {
    fn add_search_results(&self, app: &AppHandle, response: &SearchResponse) {
        let mut scope = self.0.lock().unwrap_or_else(|e| e.into_inner());
        for result in &response.results {
            let Ok(path) = std::fs::canonicalize(&result.path) else {
                continue;
            };
            if !scope.paths.insert(path.clone()) {
                continue;
            }
            scope.order.push_back(path.clone());
            if path.is_file() {
                let _ = app.asset_protocol_scope().allow_file(&path);
            }
        }
        while scope.order.len() > MAX_AUTHORIZED_PATHS {
            if let Some(path) = scope.order.pop_front() {
                scope.paths.remove(&path);
                if path.is_file() {
                    let _ = app.asset_protocol_scope().forbid_file(&path);
                }
            }
        }
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let canonical =
            std::fs::canonicalize(path).map_err(|e| format!("result path is unavailable: {e}"))?;
        let scope = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if scope.paths.contains(&canonical) {
            Ok(canonical)
        } else {
            Err("path was not returned by findr search".into())
        }
    }
}

#[derive(Serialize)]
pub struct PreviewText {
    text: String,
    truncated: bool,
}

#[tauri::command]
pub async fn search(
    app: AppHandle,
    query: String,
    limit: usize,
    no_semantic: Option<bool>,
) -> Result<SearchResponse, String> {
    let response = findr_client::search(&app, &query, limit, no_semantic.unwrap_or(false)).await?;
    app.state::<AuthorizedPaths>()
        .add_search_results(&app, &response);
    Ok(response)
}

#[tauri::command]
pub async fn get_recent_files(app: AppHandle, limit: usize) -> Result<SearchResponse, String> {
    let response = findr_client::recent_files(&app, limit).await?;
    app.state::<AuthorizedPaths>()
        .add_search_results(&app, &response);
    Ok(response)
}

#[tauri::command]
pub async fn read_preview_text(app: AppHandle, path: String) -> Result<PreviewText, String> {
    let path = app.state::<AuthorizedPaths>().resolve(&path)?;
    tauri::async_runtime::spawn_blocking(move || read_preview_file(&path))
        .await
        .map_err(|e| format!("preview task failed: {e}"))?
}

fn read_preview_file(path: &Path) -> Result<PreviewText, String> {
    if !path.is_file() {
        return Err("preview path is not a regular file".into());
    }
    let mut bytes = Vec::with_capacity(MAX_PREVIEW_BYTES as usize + 1);
    std::fs::File::open(path)
        .map_err(|e| format!("failed to open preview: {e}"))?
        .take(MAX_PREVIEW_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("failed to read preview: {e}"))?;
    let truncated = bytes.len() > MAX_PREVIEW_BYTES as usize;
    bytes.truncate(MAX_PREVIEW_BYTES as usize);
    Ok(PreviewText {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        truncated,
    })
}

#[tauri::command]
pub fn open_result(app: AppHandle, path: String) -> Result<(), String> {
    let path = app.state::<AuthorizedPaths>().resolve(&path)?;
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reveal_result(app: AppHandle, path: String) -> Result<(), String> {
    let path = app.state::<AuthorizedPaths>().resolve(&path)?;
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut child = std::process::Command::new("/usr/bin/pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to start pbcopy: {e}"))?;
        child
            .stdin
            .take()
            .ok_or_else(|| "pbcopy stdin unavailable".to_string())?
            .write_all(text.as_bytes())
            .map_err(|e| format!("failed to write clipboard: {e}"))?;
        let status = child
            .wait()
            .map_err(|e| format!("failed to wait for pbcopy: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("pbcopy exited with {status}"))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("clipboard is currently supported only on macOS".into())
    }
}

#[tauri::command]
pub async fn track_interaction(app: AppHandle, path: String, action: String) -> Result<(), String> {
    findr_client::track(&app, &path, &action).await
}

#[tauri::command]
pub async fn get_index_status(app: AppHandle) -> Result<IndexStatus, String> {
    findr_client::index_status(&app).await
}

#[tauri::command]
pub fn get_index_activity(app: AppHandle) -> IndexActivity {
    app.state::<IndexActivityState>().snapshot()
}

fn parse_macos_major_version(version: &str) -> Option<u32> {
    version.trim().split('.').next()?.parse().ok()
}

#[tauri::command]
pub fn uses_legacy_opaque_overlay() -> bool {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/usr/bin/sw_vers")
            .args(["-productVersion"])
            .output();
        output
            .ok()
            .filter(|result| result.status.success())
            .and_then(|result| String::from_utf8(result.stdout).ok())
            .and_then(|version| parse_macos_major_version(&version))
            .is_none_or(|major| major < 14)
    }
    #[cfg(not(target_os = "macos"))]
    false
}

#[tauri::command]
pub async fn get_findr_version(app: AppHandle) -> Result<String, String> {
    findr_client::version(&app).await
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(panel) = app.get_webview_panel("main") {
            panel.hide();
            return;
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        if let Err(e) = window.hide() {
            eprintln!("[commands] failed to hide main window: {e}");
        }
    }
}

#[tauri::command]
pub async fn get_doctor_report(app: AppHandle) -> Result<DoctorReport, String> {
    findr_client::doctor(&app).await
}

#[tauri::command]
pub async fn add_scan_path(app: AppHandle, path: String) -> Result<String, String> {
    findr_client::add_path(&app, &path).await
}

#[tauri::command]
pub async fn remove_scan_path(app: AppHandle, path: String) -> Result<String, String> {
    findr_client::remove_path(&app, &path).await
}

#[tauri::command]
pub async fn run_reindex(app: AppHandle) -> Result<String, String> {
    // Acquire sync lock to prevent race with daemon
    let lock = app.try_state::<SyncLock>();
    if let Some(ref lock) = lock {
        if !lock.try_acquire() {
            return Err("sync already in progress".into());
        }
    }
    let activity = app.state::<IndexActivityState>();
    activity.update(&app, "indexing", "Rebuilding search index…", true);
    let result = findr_client::rebuild(&app, None, None).await;
    if let Some(ref lock) = lock {
        lock.release();
    }
    match result {
        Ok(output) => {
            activity.update(&app, "ready", "Search index rebuilt.", false);
            Ok(output)
        }
        Err(error) => {
            activity.update(&app, "error", "Search rebuild failed.", false);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn run_sync(app: AppHandle) -> Result<String, String> {
    // Acquire sync lock to prevent race with daemon
    let lock = app.try_state::<SyncLock>();
    if let Some(ref lock) = lock {
        if !lock.try_acquire() {
            return Err("sync already in progress".into());
        }
    }
    let activity = app.state::<IndexActivityState>();
    activity.update(
        &app,
        "indexing",
        "Preparing search — finding your files…",
        true,
    );
    let result = match findr_client::sync(&app).await {
        Err(error) if findr_client::is_corrupt_index_error(&error) => {
            activity.update(&app, "indexing", "Repairing search index…", true);
            findr_client::rebuild(&app, None, None).await
        }
        result => result,
    };
    if let Some(ref lock) = lock {
        lock.release();
    }
    match result {
        Ok(output) => {
            activity.update(&app, "ready", "Search is ready.", false);
            Ok(output)
        }
        Err(error) => {
            activity.update(&app, "error", "Search setup failed.", false);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn set_api_key(app: AppHandle, key: String) -> Result<String, String> {
    findr_client::set_key(&app, &key).await
}

#[tauri::command]
pub async fn get_api_key_status(app: AppHandle) -> Result<String, String> {
    findr_client::get_key_status(&app).await
}

#[tauri::command]
pub fn get_home_dir(app: AppHandle) -> Result<String, String> {
    app.path()
        .home_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|e| format!("could not determine home directory: {e}"))
}

#[tauri::command]
pub fn get_autostart_status(app: AppHandle) -> Result<bool, String> {
    let manager = app.state::<AutoLaunchManager>();
    manager.is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.state::<AutoLaunchManager>();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn get_theme(app: AppHandle) -> Result<String, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let theme = store
        .get("theme")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "dark".into());
    Ok(theme)
}

#[tauri::command]
pub fn set_theme(app: AppHandle, theme: String) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("theme", serde_json::json!(theme));
    let _ = app.emit("theme-changed", &theme);
    Ok(())
}

#[tauri::command]
pub fn move_to_trash(app: AppHandle, path: String) -> Result<(), String> {
    let path = app.state::<AuthorizedPaths>().resolve(&path)?;
    trash::delete(&path).map_err(|e| format!("failed to trash: {e}"))
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        if let Err(e) = window.show() {
            eprintln!("[commands] failed to show settings window: {e}");
        }
        if let Err(e) = window.set_focus() {
            eprintln!("[commands] failed to focus settings window: {e}");
        }
    }
}

#[tauri::command]
pub async fn get_license_state(app: AppHandle) -> Result<LicenseState, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let state: LicenseState = store
        .get("license")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    // Use cached validation to avoid blocking on every poll
    let cache = app.try_state::<ValidationCacheState>();
    let checked_status = match cache {
        Some(ref c) => {
            // key is not persisted — pass None. Validation will use activation_id only
            // if key is needed for validation and unavailable, grace period logic handles it
            license::check_license_state_cached(&state, c, None)
        }
        None => license::check_license_state(&state),
    };

    if checked_status != state.status {
        let mut updated = state.clone();
        updated.status = checked_status.clone();
        if checked_status == LicenseStatus::Active {
            updated.validated_at = Some(chrono::Utc::now().to_rfc3339());
        }
        let value = serde_json::to_value(&updated).map_err(|e| e.to_string())?;
        store.set("license", value);
        return Ok(updated);
    }
    Ok(state)
}

#[tauri::command]
pub async fn activate_license(app: AppHandle, key: String) -> Result<LicenseState, String> {
    // Run blocking HTTP call on a blocking thread
    let state = tokio::task::spawn_blocking(move || license::activate_license(&key))
        .await
        .map_err(|e| format!("spawn_blocking failed: {e}"))??;

    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    // key is skip_serializing so it won't be written to disk
    let value = serde_json::to_value(&state).map_err(|e| e.to_string())?;
    store.set("license", value);
    Ok(state)
}

#[tauri::command]
pub async fn start_trial(app: AppHandle) -> Result<LicenseState, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let state = license::start_trial();
    let value = serde_json::to_value(&state).map_err(|e| e.to_string())?;
    store.set("license", value);
    Ok(state)
}

#[tauri::command]
pub fn get_trial_days_remaining(app: AppHandle) -> Result<i64, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let state: LicenseState = store
        .get("license")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(license::trial_days_remaining(&state))
}

#[cfg(test)]
mod tests {
    use super::parse_macos_major_version;

    #[test]
    fn parses_macos_major_version() {
        assert_eq!(parse_macos_major_version("12.7.6\n"), Some(12));
        assert_eq!(parse_macos_major_version("15.5"), Some(15));
        assert_eq!(parse_macos_major_version("unknown"), None);
    }
}
