use crate::findr_client::{self, IndexStatus, SearchResponse};
use crate::license::{self, LicenseState, LicenseStatus};
use tauri::{AppHandle, Manager};
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
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn get_license_state(app: AppHandle) -> Result<LicenseState, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let state: LicenseState = store
        .get("license")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let checked_status = license::check_license_state(&state);
    if checked_status != state.status {
        let mut updated = state.clone();
        updated.status = checked_status.clone();
        if checked_status == LicenseStatus::Active {
            updated.validated_at = Some(chrono::Utc::now().to_rfc3339());
        }
        store.set("license", serde_json::to_value(&updated).unwrap());
        return Ok(updated);
    }
    Ok(state)
}

#[tauri::command]
pub fn activate_license(app: AppHandle, key: String) -> Result<LicenseState, String> {
    let state = license::activate_license(&key)?;
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("license", serde_json::to_value(&state).unwrap());
    Ok(state)
}

#[tauri::command]
pub fn start_trial(app: AppHandle) -> Result<LicenseState, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let state = license::start_trial();
    store.set("license", serde_json::to_value(&state).unwrap());
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
