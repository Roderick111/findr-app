use crate::background::SyncLock;
use crate::findr_client::{self, DoctorReport, IndexStatus, SearchResponse};
use crate::license::{self, LicenseState, LicenseStatus, ValidationCacheState};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::AutoLaunchManager;
use tauri_plugin_store::StoreExt;

#[tauri::command]
pub async fn search(
    app: AppHandle,
    query: String,
    limit: usize,
    no_semantic: bool,
) -> Result<SearchResponse, String> {
    findr_client::search(&app, &query, limit, no_semantic).await
}

#[tauri::command]
pub async fn get_recent_files(
    app: AppHandle,
    limit: usize,
) -> Result<SearchResponse, String> {
    findr_client::recent_files(&app, limit).await
}

#[tauri::command]
pub async fn track_interaction(
    app: AppHandle,
    path: String,
    action: String,
) -> Result<(), String> {
    findr_client::track(&app, &path, &action).await
}

#[tauri::command]
pub async fn get_index_status(app: AppHandle) -> Result<IndexStatus, String> {
    findr_client::index_status(&app).await
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
    let report = findr_client::doctor(&app).await?;
    let remaining: Vec<&str> = report
        .scan_paths
        .iter()
        .map(|p| p.path.as_str())
        .filter(|p| *p != path)
        .collect();
    if remaining.is_empty() {
        return Err("cannot remove last scan path".into());
    }
    let paths_str = remaining.join(",");
    findr_client::rebuild(&app, None, Some(&paths_str)).await
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
    let result = findr_client::rebuild(&app, None, None).await;
    if let Some(ref lock) = lock {
        lock.release();
    }
    result
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
    let result = findr_client::sync(&app).await;
    if let Some(ref lock) = lock {
        lock.release();
    }
    result
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
pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "could not determine home directory".to_string())
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
pub fn move_to_trash(path: String) -> Result<(), String> {
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
