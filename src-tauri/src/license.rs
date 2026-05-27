use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

const POLAR_ORG_ID: &str = "499639ab-c131-4dc7-9fe7-a4cde74f56f4";
const TRIAL_DAYS: i64 = 14;
const OFFLINE_GRACE_DAYS: i64 = 7;
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const VALIDATION_CACHE_SECS: i64 = 3600; // 1 hour

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LicenseState {
    pub status: LicenseStatus,
    // key is transient only — never persisted to disk.
    // Kept for backwards-compatible deserialization of old settings.
    #[serde(skip_serializing, default)]
    #[allow(dead_code)]
    pub key: Option<String>,
    pub activated_at: Option<String>,
    pub validated_at: Option<String>,
    pub activation_id: Option<String>,
    pub trial_started_at: Option<String>,
}

/// Cached validation result to avoid re-validating on every settings poll.
#[derive(Default)]
pub struct ValidationCache {
    pub last_checked: Option<chrono::DateTime<chrono::Utc>>,
    pub last_result: Option<LicenseStatus>,
}

/// Managed state wrapper for the validation cache.
pub struct ValidationCacheState(pub Mutex<ValidationCache>);

impl Default for ValidationCacheState {
    fn default() -> Self {
        Self(Mutex::new(ValidationCache::default()))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    Active,
    Trial,
    TrialExpired,
    Invalid,
    Unknown,
}

#[derive(Deserialize, Debug)]
struct PolarActivateResponse {
    id: String,
    license_key: PolarLicenseKey,
}

#[derive(Deserialize, Debug)]
struct PolarLicenseKey {
    status: String,
}

#[derive(Deserialize, Debug)]
struct PolarValidateResponse {
    status: String,
}

impl Default for LicenseState {
    fn default() -> Self {
        Self {
            status: LicenseStatus::Unknown,
            key: None,
            activated_at: None,
            validated_at: None,
            activation_id: None,
            trial_started_at: None,
        }
    }
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(HTTP_TIMEOUT)
        .timeout_read(HTTP_TIMEOUT)
        .timeout_write(HTTP_TIMEOUT)
        .build()
}

/// Activate a license key. Returns state with key populated transiently
/// (key is skip_serializing so it won't be persisted).
pub fn activate_license(key: &str) -> Result<LicenseState, String> {
    let fingerprint = machine_fingerprint().map_err(|e| format!("fingerprint failed: {e}"))?;
    let label = format!("findr-desktop-{}", &fingerprint[..8]);

    let agent = http_agent();
    let resp: ureq::Response = agent
        .post("https://api.polar.sh/v1/customer-portal/license-keys/activate")
        .send_json(serde_json::json!({
            "key": key,
            "organization_id": POLAR_ORG_ID,
            "label": label,
            "conditions": { "fingerprint": fingerprint },
        }))
        .map_err(|e| format!("activation request failed: {e}"))?;

    let parsed: PolarActivateResponse = resp
        .into_json()
        .map_err(|e| format!("parse activation response: {e}"))?;

    if parsed.license_key.status != "granted" {
        return Err(format!("license status: {}", parsed.license_key.status));
    }

    let now = chrono::Utc::now().to_rfc3339();
    Ok(LicenseState {
        status: LicenseStatus::Active,
        key: None, // not persisted
        activated_at: Some(now.clone()),
        validated_at: Some(now),
        activation_id: Some(parsed.id),
        trial_started_at: None,
    })
}

pub fn validate_license(key: &str, activation_id: &str) -> Result<LicenseStatus, String> {
    let agent = http_agent();
    let resp: ureq::Response = agent
        .post("https://api.polar.sh/v1/customer-portal/license-keys/validate")
        .send_json(serde_json::json!({
            "key": key,
            "organization_id": POLAR_ORG_ID,
            "activation_id": activation_id,
        }))
        .map_err(|e| format!("validation request failed: {e}"))?;

    let parsed: PolarValidateResponse = resp
        .into_json()
        .map_err(|e| format!("parse validate response: {e}"))?;

    match parsed.status.as_str() {
        "granted" => Ok(LicenseStatus::Active),
        "revoked" | "disabled" => Ok(LicenseStatus::Invalid),
        _ => Ok(LicenseStatus::Invalid),
    }
}

/// Check license state. No longer calls validate_license directly —
/// that is done via check_license_state_cached which respects the cache.
pub fn check_license_state(state: &LicenseState) -> LicenseStatus {
    match &state.status {
        LicenseStatus::Active => {
            // If we have a validated_at timestamp, check offline grace period
            if let Some(validated_at) = &state.validated_at {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(validated_at) {
                    let age = chrono::Utc::now() - ts.with_timezone(&chrono::Utc);
                    if age.num_days() > OFFLINE_GRACE_DAYS {
                        // Grace period expired and we can't validate — restrict
                        // The caller (commands.rs) handles actual re-validation
                        // via the cached path. Here we just report the grace expiry.
                        return LicenseStatus::Invalid;
                    }
                }
            }
            LicenseStatus::Active
        }
        LicenseStatus::Trial => check_trial(state),
        _ => state.status.clone(),
    }
}

/// Cached validation: only re-validates at most once per VALIDATION_CACHE_SECS.
/// key is needed for validation but is NOT persisted — must be passed in if available.
pub fn check_license_state_cached(
    state: &LicenseState,
    cache: &ValidationCacheState,
    key: Option<&str>,
) -> LicenseStatus {
    match &state.status {
        LicenseStatus::Active => {
            // Check cache first
            if let Ok(cache_guard) = cache.0.lock() {
                if let Some(last_checked) = cache_guard.last_checked {
                    let age = chrono::Utc::now() - last_checked;
                    if age.num_seconds() < VALIDATION_CACHE_SECS {
                        // Cache is fresh, return cached result
                        if let Some(ref result) = cache_guard.last_result {
                            return result.clone();
                        }
                    }
                }
            }

            // Check if we need to re-validate
            if let Some(validated_at) = &state.validated_at {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(validated_at) {
                    let age = chrono::Utc::now() - ts.with_timezone(&chrono::Utc);
                    if age.num_days() > OFFLINE_GRACE_DAYS {
                        // Try to validate if we have key + activation_id
                        if let (Some(k), Some(aid)) = (key, &state.activation_id) {
                            match validate_license(k, aid) {
                                Ok(status) => {
                                    // Update cache
                                    if let Ok(mut cache_guard) = cache.0.lock() {
                                        cache_guard.last_checked = Some(chrono::Utc::now());
                                        cache_guard.last_result = Some(status.clone());
                                    }
                                    return status;
                                }
                                Err(_) => {
                                    // Validation failed (network error) — keep current status unchanged
                                    // but mark cache so we don't retry immediately
                                    if let Ok(mut cache_guard) = cache.0.lock() {
                                        cache_guard.last_checked = Some(chrono::Utc::now());
                                        cache_guard.last_result = Some(state.status.clone());
                                    }
                                    return state.status.clone();
                                }
                            }
                        }
                        // No key available — grace period expired, restrict
                        return LicenseStatus::Invalid;
                    }
                }
            }
            LicenseStatus::Active
        }
        LicenseStatus::Trial => check_trial(state),
        _ => state.status.clone(),
    }
}

pub fn start_trial() -> LicenseState {
    LicenseState {
        status: LicenseStatus::Trial,
        trial_started_at: Some(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }
}

pub fn trial_days_remaining(state: &LicenseState) -> i64 {
    if let Some(started) = &state.trial_started_at {
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(started) {
            let elapsed = chrono::Utc::now() - ts.with_timezone(&chrono::Utc);
            return (TRIAL_DAYS - elapsed.num_days()).max(0);
        }
    }
    0
}

fn check_trial(state: &LicenseState) -> LicenseStatus {
    if trial_days_remaining(state) > 0 {
        LicenseStatus::Trial
    } else {
        LicenseStatus::TrialExpired
    }
}

fn machine_fingerprint() -> Result<String, String> {
    let raw = platform_id()?;
    let hash = Sha256::digest(raw.as_bytes());
    let hex: String = hash[..16].iter().map(|b| format!("{b:02x}")).collect();
    Ok(hex)
}

#[cfg(target_os = "macos")]
fn platform_id() -> Result<String, String> {
    let output = Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .map_err(|e| format!("ioreg failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("IOPlatformUUID") {
            if let Some(uuid) = line.split('"').nth(3) {
                return Ok(uuid.to_string());
            }
        }
    }
    Err("IOPlatformUUID not found".into())
}

#[cfg(target_os = "windows")]
fn platform_id() -> Result<String, String> {
    let output = Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Cryptography",
            "/v",
            "MachineGuid",
        ])
        .output()
        .map_err(|e| format!("reg query failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("MachineGuid") {
            if let Some(guid) = line.split_whitespace().last() {
                return Ok(guid.to_string());
            }
        }
    }
    Err("MachineGuid not found".into())
}

#[cfg(target_os = "linux")]
fn platform_id() -> Result<String, String> {
    std::fs::read_to_string("/etc/machine-id")
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("/etc/machine-id: {e}"))
}
