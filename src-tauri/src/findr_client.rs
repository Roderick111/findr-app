use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

/// Max accumulated stdout/stderr size: 10 MB.
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

/// Query timeout. Interactive queries must fail fast.
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);
/// Maintenance timeout. Indexing a home directory can legitimately take minutes.
const MAINTENANCE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Max chars of raw output shown in error messages to avoid information disclosure.
const ERROR_PREVIEW_CHARS: usize = 200;

#[derive(Default)]
pub struct SearchProcessState {
    next_id: AtomicU64,
    active: Mutex<Option<(u64, CommandChild)>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SearchResponse {
    pub query: String,
    pub mode: String,
    pub elapsed_ms: u64,
    pub total_results: u64,
    pub results: Vec<SearchResult>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub hint: Option<String>,
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
    #[serde(default)]
    pub custom: bool,
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

fn parse_search_response(stdout: &str) -> Result<SearchResponse, String> {
    let response =
        serde_json::from_str::<SearchResponse>(stdout).map_err(|e| parse_error_msg(&e, stdout))?;
    if response.mode == "error" {
        return Err(response
            .error
            .or(response.hint)
            .unwrap_or_else(|| "findr returned an unspecified error".into()));
    }
    Ok(response)
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
        "--no-sync",
    ];
    if no_semantic {
        args.push("--no-semantic");
    }

    run_findr_with_timeout(app, &args, &[], QUERY_TIMEOUT, true)
        .await
        .and_then(|stdout| parse_search_response(&stdout))
}

pub async fn recent_files(app: &AppHandle, limit: usize) -> Result<SearchResponse, String> {
    let limit_str = limit.to_string();
    let args = vec!["search", "", "--json", "--limit", &limit_str, "--no-sync"];
    run_findr_with_timeout(app, &args, &[], QUERY_TIMEOUT, true)
        .await
        .and_then(|stdout| parse_search_response(&stdout))
}

pub async fn track(app: &AppHandle, path: &str, action: &str) -> Result<(), String> {
    run_findr(app, &["track", path, "--action", action], &[])
        .await
        .map(|_| ())
}

pub async fn index_status(app: &AppHandle) -> Result<IndexStatus, String> {
    let stdout = run_findr(app, &["index", "status", "--json"], &[]).await?;
    serde_json::from_str::<IndexStatus>(&stdout).map_err(|e| parse_error_msg(&e, &stdout))
}

pub async fn version(app: &AppHandle) -> Result<String, String> {
    let stdout = run_findr(app, &["--version"], &[]).await?;
    Ok(stdout.trim().to_string())
}

pub async fn doctor(app: &AppHandle) -> Result<DoctorReport, String> {
    let stdout = run_findr(app, &["doctor", "--json"], &[]).await?;
    serde_json::from_str::<DoctorReport>(&stdout).map_err(|e| parse_error_msg(&e, &stdout))
}

pub async fn add_path(app: &AppHandle, path: &str) -> Result<String, String> {
    run_findr(app, &["index", "add-path", path], &[]).await
}

pub async fn remove_path(app: &AppHandle, path: &str) -> Result<String, String> {
    run_findr(app, &["index", "remove-path", path], &[]).await
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
    run_findr(app, &["config", "set-key"], &[("OPENROUTER_API_KEY", key)]).await
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
    run_findr_with_timeout(app, args, env_vars, MAINTENANCE_TIMEOUT, false).await
}

fn take_query_child(app: &AppHandle, query_id: u64) -> Option<CommandChild> {
    let state = app.state::<SearchProcessState>();
    let mut active = state.active.lock().unwrap_or_else(|e| e.into_inner());
    if active
        .as_ref()
        .is_some_and(|(active_id, _)| *active_id == query_id)
    {
        active.take().map(|(_, child)| child)
    } else {
        None
    }
}

async fn run_findr_with_timeout(
    app: &AppHandle,
    args: &[&str],
    env_vars: &[(&str, &str)],
    timeout: Duration,
    cancel_previous_query: bool,
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

    let mut local_child = None;
    let query_id = if cancel_previous_query {
        let state = app.state::<SearchProcessState>();
        let id = state.next_id.fetch_add(1, Ordering::Relaxed);
        let previous = {
            let mut active = state.active.lock().unwrap_or_else(|e| e.into_inner());
            active.replace((id, child))
        };
        if let Some((_, previous_child)) = previous {
            let _ = previous_child.kill();
        }
        Some(id)
    } else {
        local_child = Some(child);
        None
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code: Option<i32> = None;
    let mut output_exceeded = false;

    let recv_result = tokio::time::timeout(timeout, async {
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
        let child = query_id
            .and_then(|id| take_query_child(app, id))
            .or_else(|| local_child.take());
        if let Some(child) = child {
            let _ = child.kill();
        }
        return Err(format!("findr timed out after {}s", timeout.as_secs()));
    }

    if let Some(id) = query_id {
        drop(take_query_child(app, id));
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── truncate_chars ──────────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate_chars("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate_chars("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_string_cut() {
        assert_eq!(truncate_chars("hello world", 5), "hello");
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate_chars("", 10), "");
    }

    #[test]
    fn truncate_zero_max() {
        assert_eq!(truncate_chars("hello", 0), "");
    }

    #[test]
    fn truncate_multibyte_utf8_no_panic() {
        // Each emoji is multiple bytes but one char
        let input = "\u{1F600}\u{1F601}\u{1F602}\u{1F603}"; // 4 emoji chars
        let result = truncate_chars(input, 2);
        assert_eq!(result.chars().count(), 2);
        assert_eq!(result, "\u{1F600}\u{1F601}");
    }

    #[test]
    fn truncate_cjk_characters() {
        let input = "\u{4F60}\u{597D}\u{4E16}\u{754C}"; // 你好世界
        let result = truncate_chars(input, 2);
        assert_eq!(result, "\u{4F60}\u{597D}");
    }

    // ── parse_error_msg ─────────────────────────────────────────────

    #[test]
    fn parse_error_msg_short_output() {
        let msg = parse_error_msg(&"bad json", "short output");
        assert!(msg.contains("bad json"));
        assert!(msg.contains("short output"));
    }

    #[test]
    fn parse_error_msg_truncates_long_output() {
        let long_output = "x".repeat(500);
        let msg = parse_error_msg(&"parse fail", &long_output);
        assert!(msg.contains("parse fail"));
        // Preview should be truncated to ERROR_PREVIEW_CHARS
        let preview_start = msg.find("(preview: ").unwrap() + "(preview: ".len();
        let preview_end = msg.rfind(')').unwrap();
        let preview = &msg[preview_start..preview_end];
        assert_eq!(preview.len(), ERROR_PREVIEW_CHARS);
    }

    #[test]
    fn search_error_envelope_is_rejected() {
        let json = r#"{
            "query":"x",
            "mode":"error",
            "elapsed_ms":0,
            "total_results":0,
            "results":[],
            "error":"index corrupt",
            "hint":"rebuild"
        }"#;
        let err = parse_search_response(json).unwrap_err();
        assert!(err.contains("index corrupt"));
    }

    #[test]
    fn parse_error_msg_empty_output() {
        let msg = parse_error_msg(&"err", "");
        assert!(msg.contains("err"));
        assert!(msg.contains("preview: "));
    }

    // ── SearchResponse deserialization ───────────────────────────────

    #[test]
    fn search_response_parses_valid_json() {
        let json = r#"{
            "query": "test",
            "mode": "hybrid",
            "elapsed_ms": 42,
            "total_results": 2,
            "results": [
                {
                    "path": "/home/user/file.txt",
                    "filename": "file.txt",
                    "score": 0.95,
                    "match_type": "content",
                    "size_bytes": 1024,
                    "modified": "2026-01-01T00:00:00Z",
                    "file_type": "text",
                    "content_snippet": "matching content",
                    "is_dir": false,
                    "interactions": 5
                },
                {
                    "path": "/home/user/dir",
                    "filename": "dir",
                    "score": 0.7,
                    "match_type": "filename",
                    "is_dir": true,
                    "interactions": 0
                }
            ]
        }"#;

        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.query, "test");
        assert_eq!(resp.mode, "hybrid");
        assert_eq!(resp.elapsed_ms, 42);
        assert_eq!(resp.total_results, 2);
        assert_eq!(resp.results.len(), 2);

        let r0 = &resp.results[0];
        assert_eq!(r0.path, "/home/user/file.txt");
        assert_eq!(r0.filename, "file.txt");
        assert!((r0.score - 0.95).abs() < f64::EPSILON);
        assert_eq!(r0.size_bytes, Some(1024));
        assert_eq!(r0.content_snippet, Some("matching content".into()));
        assert!(!r0.is_dir);
        assert_eq!(r0.interactions, 5);

        let r1 = &resp.results[1];
        assert!(r1.is_dir);
        assert!(r1.size_bytes.is_none());
        assert!(r1.content_snippet.is_none());
    }

    #[test]
    fn search_result_optional_fields_default_to_none() {
        let json = r#"{
            "path": "/test",
            "filename": "test",
            "score": 1.0,
            "match_type": "name",
            "is_dir": false,
            "interactions": 0
        }"#;
        let result: SearchResult = serde_json::from_str(json).unwrap();
        assert!(result.size_bytes.is_none());
        assert!(result.modified.is_none());
        assert!(result.file_type.is_none());
        assert!(result.content_snippet.is_none());
    }

    #[test]
    fn search_response_malformed_json_errors() {
        let bad_json = r#"{"query": "test", "mode": }"#;
        let result = serde_json::from_str::<SearchResponse>(bad_json);
        assert!(result.is_err());
    }

    #[test]
    fn search_response_missing_required_field_errors() {
        // Missing "results" field
        let json = r#"{"query": "test", "mode": "hybrid", "elapsed_ms": 0, "total_results": 0}"#;
        let result = serde_json::from_str::<SearchResponse>(json);
        assert!(result.is_err());
    }

    // ── DoctorReport deserialization ────────────────────────────────

    #[test]
    fn doctor_report_parses_valid_json() {
        let json = r#"{
            "version": "1.2.3",
            "database": {
                "ok": true,
                "path": "/home/.findr/db",
                "size_bytes": 5000000,
                "files_indexed": 1234,
                "content_indexed": 500,
                "last_updated": "2026-01-01T00:00:00Z",
                "last_full_reindex": null
            },
            "ocr": {
                "binary_found": true,
                "total_images": 100,
                "ocr_completed": 80
            },
            "hnsw": {
                "index_exists": true,
                "vector_count": 500
            },
            "content_index": {
                "path": "/home/.findr/content",
                "size_bytes": 2000000
            },
            "index_location": "/home/.findr",
            "scan_paths": [
                {"path": "/home/user", "exists": true},
                {"path": "/tmp/gone", "exists": false}
            ],
            "permissions": {
                "ok": true,
                "inaccessible": []
            },
            "os": {
                "os": "macos",
                "arch": "aarch64"
            }
        }"#;

        let report: DoctorReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.version, "1.2.3");
        assert!(report.database.ok);
        assert_eq!(report.database.files_indexed, 1234);
        assert!(report.ocr.binary_found);
        assert!(report.hnsw.index_exists);
        assert_eq!(report.scan_paths.len(), 2);
        assert!(report.scan_paths[0].exists);
        assert!(!report.scan_paths[1].exists);
        assert!(report.permissions.ok);
        assert_eq!(report.os.os, "macos");
        assert_eq!(report.index_location, Some("/home/.findr".into()));
    }

    #[test]
    fn doctor_report_optional_fields_can_be_absent() {
        let json = r#"{
            "version": "1.0.0",
            "database": {
                "ok": true,
                "path": "/db",
                "size_bytes": 0,
                "files_indexed": 0,
                "content_indexed": 0,
                "last_updated": null,
                "last_full_reindex": null
            },
            "ocr": {"binary_found": false, "total_images": 0, "ocr_completed": 0},
            "hnsw": {"index_exists": false, "vector_count": 0},
            "content_index": {"path": "/ci", "size_bytes": 0},
            "scan_paths": [],
            "permissions": {"ok": true},
            "os": {"os": "linux", "arch": "x86_64"}
        }"#;

        let report: DoctorReport = serde_json::from_str(json).unwrap();
        assert!(report.index_location.is_none());
        assert!(report.recent_errors.is_none());
        assert!(report.permissions.inaccessible.is_empty());
    }

    #[test]
    fn doctor_report_malformed_json_errors() {
        let result = serde_json::from_str::<DoctorReport>("not json");
        assert!(result.is_err());
    }

    // ── IndexStatus deserialization ─────────────────────────────────

    #[test]
    fn index_status_parses_with_extra_fields() {
        let json = r#"{
            "files_indexed": 100,
            "content_indexed": 50,
            "last_sync": "2026-01-01T00:00:00Z",
            "some_new_field": "future-proof"
        }"#;
        let status: IndexStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.files_indexed, Some(100));
        assert_eq!(status.content_indexed, Some(50));
        assert_eq!(status.last_sync, Some("2026-01-01T00:00:00Z".into()));
        // Extra fields captured in `other` via serde(flatten)
        assert_eq!(status.other.get("some_new_field").unwrap(), "future-proof");
    }

    #[test]
    fn index_status_all_optional_fields_default() {
        let json = r#"{}"#;
        let status: IndexStatus = serde_json::from_str(json).unwrap();
        assert!(status.files_indexed.is_none());
        assert!(status.content_indexed.is_none());
        assert!(status.last_sync.is_none());
    }

    // ── SearchResponse serialization roundtrip ──────────────────────

    #[test]
    fn search_response_serialize_deserialize_roundtrip() {
        let original = SearchResponse {
            query: "test query".into(),
            mode: "hybrid".into(),
            elapsed_ms: 100,
            total_results: 1,
            results: vec![SearchResult {
                path: "/test/path".into(),
                filename: "path".into(),
                score: 0.85,
                match_type: "content".into(),
                size_bytes: Some(512),
                modified: Some("2026-05-27T00:00:00Z".into()),
                file_type: Some("text".into()),
                content_snippet: None,
                is_dir: false,
                interactions: 3,
            }],
            error: None,
            hint: None,
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: SearchResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.query, original.query);
        assert_eq!(restored.total_results, original.total_results);
        assert_eq!(restored.results.len(), 1);
        assert_eq!(restored.results[0].path, "/test/path");
        assert_eq!(restored.results[0].interactions, 3);
    }
}
