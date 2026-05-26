use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

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

    run_findr(app, &args).await.and_then(|stdout| {
        serde_json::from_str::<SearchResponse>(&stdout)
            .map_err(|e| format!("parse error: {} (stdout: {})", e, &stdout[..stdout.len().min(200)]))
    })
}

pub async fn recent_files(app: &AppHandle, limit: usize) -> Result<SearchResponse, String> {
    let limit_str = limit.to_string();
    let args = vec!["search", "", "--json", "--limit", &limit_str];
    run_findr(app, &args).await.and_then(|stdout| {
        serde_json::from_str::<SearchResponse>(&stdout)
            .map_err(|e| format!("parse error: {} (stdout: {})", e, &stdout[..stdout.len().min(200)]))
    })
}

pub async fn track(app: &AppHandle, path: &str, action: &str) -> Result<(), String> {
    run_findr(app, &["track", path, "--action", action]).await.map(|_| ())
}

pub async fn index_status(app: &AppHandle) -> Result<IndexStatus, String> {
    let stdout = run_findr(app, &["index", "status", "--json"]).await?;
    serde_json::from_str::<IndexStatus>(&stdout)
        .map_err(|e| format!("parse error: {} (stdout: {})", e, &stdout[..stdout.len().min(200)]))
}

pub async fn version(app: &AppHandle) -> Result<String, String> {
    let stdout = run_findr(app, &["--version"]).await?;
    Ok(stdout.trim().to_string())
}

async fn run_findr(app: &AppHandle, args: &[&str]) -> Result<String, String> {
    let sidecar = app
        .shell()
        .sidecar("findr")
        .map_err(|e| format!("sidecar lookup failed: {}", e))?;

    let (mut rx, _child) = sidecar
        .args(args)
        .spawn()
        .map_err(|e| format!("sidecar spawn failed: {}", e))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code: Option<i32> = None;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => stdout.push_str(&String::from_utf8_lossy(&bytes)),
            CommandEvent::Stderr(bytes) => stderr.push_str(&String::from_utf8_lossy(&bytes)),
            CommandEvent::Terminated(payload) => exit_code = payload.code,
            _ => {}
        }
    }

    match exit_code {
        Some(0) => Ok(stdout),
        Some(code) => Err(format!("findr exited {} (stderr: {})", code, stderr.trim())),
        None => Err(format!("findr terminated without exit code (stderr: {})", stderr.trim())),
    }
}
