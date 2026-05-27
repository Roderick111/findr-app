use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

/// Max accumulated stdout/stderr size: 10 MB.
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

/// Sidecar process timeout.
const SIDECAR_TIMEOUT: Duration = Duration::from_secs(30);

/// Max chars of raw output shown in error messages to avoid information disclosure.
const ERROR_PREVIEW_CHARS: usize = 200;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SearchResponse {
    pub query: String,
    pub mode: String,
    pub elapsed_ms: u64,
    pub total_results: u64,
    pub results: Vec<SearchResult>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SearchResult {
    pub path: String,
    pub filename: String,
    pub score: f64,
    pub match_type: String,
    pub size_bytes: Option<u64>,
    pub modified: Option<String>,
    pub file_type: Option<String>,
    pub content_snippet: Option<String>,
    pub is_dir: bool,
    pub interactions: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct IndexStatus {
    #[serde(default)]
    pub files_indexed: Option<u64>,
    #[serde(default)]
    pub content_indexed: Option<u64>,
    #[serde(default)]
    pub last_sync: Option<String>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DoctorReport {
    pub version: String,
    pub database: DatabaseInfo,
    pub ocr: OcrInfo,
    pub hnsw: HnswInfo,
    pub content_index: ContentIndexInfo,
    #[serde(default)]
    pub index_location: Option<String>,
    pub scan_paths: Vec<ScanPath>,
    pub permissions: PermissionsInfo,
    pub os: OsInfo,
    #[serde(default)]
    pub recent_errors: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DatabaseInfo {
    pub ok: bool,
    pub path: String,
    pub size_bytes: u64,
    pub files_indexed: u64,
    pub content_indexed: u64,
    pub last_updated: Option<String>,
    pub last_full_reindex: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OcrInfo {
    pub binary_found: bool,
    pub total_images: u64,
    pub ocr_completed: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct HnswInfo {
    pub index_exists: bool,
    pub vector_count: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ContentIndexInfo {
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ScanPath {
    pub path: String,
    pub exists: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PermissionsInfo {
    pub ok: bool,
    #[serde(default)]
    pub inaccessible: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OsInfo {
    pub os: String,
    pub arch: String,
}

/// Truncate a string to at most `max_chars` characters (UTF-8 safe).
fn truncate_chars(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect::<String>()
}

/// Build a parse-error message with truncated output preview.
fn parse_error_msg(err: &impl std::fmt::Display, raw_output: &str) -> String {
    let preview = truncate_chars(raw_output, ERROR_PREVIEW_CHARS);
    format!("Failed to parse response: {} (preview: {})", err, preview)
}

pub async fn search(
    app: &AppHandle,
    query: &str,
    limit: usize,
    no_semantic: bool,
) -> Result<SearchResponse, String> {
    let limit_str = limit.to_string();
    let mut args = vec![
        "search",
        query,
        "--json",
        "--limit",
        &limit_str,
    ];
    if no_semantic {
        args.push("--no-semantic");
    }

    run_findr(app, &args, &[]).await.and_then(|stdout| {
        serde_json::from_str::<SearchResponse>(&stdout)
            .map_err(|e| parse_error_msg(&e, &stdout))
    })
}

pub async fn recent_files(app: &AppHandle, limit: usize) -> Result<SearchResponse, String> {
    let limit_str = limit.to_string();
    let args = vec!["search", "", "--json", "--limit", &limit_str];
    run_findr(app, &args, &[]).await.and_then(|stdout| {
        serde_json::from_str::<SearchResponse>(&stdout)
            .map_err(|e| parse_error_msg(&e, &stdout))
    })
}

pub async fn track(app: &AppHandle, path: &str, action: &str) -> Result<(), String> {
    run_findr(app, &["track", path, "--action", action], &[]).await.map(|_| ())
}

pub async fn index_status(app: &AppHandle) -> Result<IndexStatus, String> {
    let stdout = run_findr(app, &["index", "status", "--json"], &[]).await?;
    serde_json::from_str::<IndexStatus>(&stdout)
        .map_err(|e| parse_error_msg(&e, &stdout))
}

pub async fn version(app: &AppHandle) -> Result<String, String> {
    let stdout = run_findr(app, &["--version"], &[]).await?;
    Ok(stdout.trim().to_string())
}

pub async fn doctor(app: &AppHandle) -> Result<DoctorReport, String> {
    let stdout = run_findr(app, &["doctor", "--json"], &[]).await?;
    serde_json::from_str::<DoctorReport>(&stdout)
        .map_err(|e| parse_error_msg(&e, &stdout))
}

pub async fn add_path(app: &AppHandle, path: &str) -> Result<String, String> {
    run_findr(app, &["index", "add-path", path], &[]).await
}

pub async fn rebuild(
    app: &AppHandle,
    preset: Option<&str>,
    paths: Option<&str>,
) -> Result<String, String> {
    let mut args = vec!["index", "rebuild"];
    if let Some(p) = preset {
        args.push("--preset");
        args.push(p);
    }
    if let Some(p) = paths {
        args.push("--paths");
        args.push(p);
    }
    run_findr(app, &args, &[]).await
}

pub async fn sync(app: &AppHandle) -> Result<String, String> {
    run_findr(app, &["index", "sync"], &[]).await
}

pub async fn set_key(app: &AppHandle, key: &str) -> Result<String, String> {
    run_findr(app, &["config", "set-key", key], &[]).await
}

pub async fn get_key_status(app: &AppHandle) -> Result<String, String> {
    let stdout = run_findr(app, &["config", "get-key"], &[]).await?;
    Ok(stdout.trim().to_string())
}

async fn run_findr(
    app: &AppHandle,
    args: &[&str],
    env_vars: &[(&str, &str)],
) -> Result<String, String> {
    let sidecar = app
        .shell()
        .sidecar("findr")
        .map_err(|e| format!("sidecar lookup failed: {}", e))?;

    let mut command = sidecar.args(args);
    for &(key, value) in env_vars {
        command = command.env(key, value);
    }

    let (mut rx, child) = command
        .spawn()
        .map_err(|e| format!("sidecar spawn failed: {}", e))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code: Option<i32> = None;
    let mut output_exceeded = false;

    let recv_result = tokio::time::timeout(SIDECAR_TIMEOUT, async {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(bytes) if !output_exceeded => {
                    let chunk = String::from_utf8_lossy(&bytes);
                    if stdout.len() + chunk.len() > MAX_OUTPUT_BYTES {
                        output_exceeded = true;
                    } else {
                        stdout.push_str(&chunk);
                    }
                }
                CommandEvent::Stderr(bytes) if !output_exceeded => {
                    let chunk = String::from_utf8_lossy(&bytes);
                    if stderr.len() + chunk.len() > MAX_OUTPUT_BYTES {
                        output_exceeded = true;
                    } else {
                        stderr.push_str(&chunk);
                    }
                }
                CommandEvent::Terminated(payload) => exit_code = payload.code,
                _ => {}
            }
        }
    })
    .await;

    if recv_result.is_err() {
        // Timeout: kill the hung sidecar process
        let _ = child.kill();
        return Err(format!(
            "findr timed out after {}s",
            SIDECAR_TIMEOUT.as_secs()
        ));
    }

    if output_exceeded {
        return Err(format!(
            "findr output exceeded {} MB limit",
            MAX_OUTPUT_BYTES / (1024 * 1024)
        ));
    }

    match exit_code {
        Some(0) => Ok(stdout),
        Some(code) => Err(format!(
            "findr exited {} (stderr: {})",
            code,
            truncate_chars(stderr.trim(), ERROR_PREVIEW_CHARS)
        )),
        None => Err(format!(
            "findr terminated without exit code (stderr: {})",
            truncate_chars(stderr.trim(), ERROR_PREVIEW_CHARS)
        )),
    }
}
