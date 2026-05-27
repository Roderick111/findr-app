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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────

    fn state_with_trial_started(days_ago: i64) -> LicenseState {
        let started = chrono::Utc::now() - chrono::Duration::days(days_ago);
        LicenseState {
            status: LicenseStatus::Trial,
            trial_started_at: Some(started.to_rfc3339()),
            ..Default::default()
        }
    }

    fn active_state_validated_days_ago(days: i64) -> LicenseState {
        let ts = chrono::Utc::now() - chrono::Duration::days(days);
        LicenseState {
            status: LicenseStatus::Active,
            validated_at: Some(ts.to_rfc3339()),
            activation_id: Some("act_test123".into()),
            ..Default::default()
        }
    }

    // ── LicenseState default ────────────────────────────────────────

    #[test]
    fn default_status_is_unknown() {
        let state = LicenseState::default();
        assert_eq!(state.status, LicenseStatus::Unknown);
    }

    #[test]
    fn default_all_optional_fields_none() {
        let state = LicenseState::default();
        assert!(state.key.is_none());
        assert!(state.activated_at.is_none());
        assert!(state.validated_at.is_none());
        assert!(state.activation_id.is_none());
        assert!(state.trial_started_at.is_none());
    }

    // ── LicenseState serialization ──────────────────────────────────

    #[test]
    fn key_field_skipped_during_serialization() {
        let state = LicenseState {
            status: LicenseStatus::Active,
            key: Some("secret-key-123".into()),
            activated_at: Some("2026-01-01T00:00:00Z".into()),
            ..Default::default()
        };
        let json = serde_json::to_value(&state).unwrap();
        // key must NOT appear in serialized output
        assert!(json.get("key").is_none(), "key field must be skip_serializing");
        // other fields must be present
        assert_eq!(json["status"], "active");
        assert_eq!(json["activated_at"], "2026-01-01T00:00:00Z");
    }

    #[test]
    fn key_field_populated_during_deserialization() {
        // Old persisted data might have a key field — backwards compat
        let json = r#"{
            "status": "active",
            "key": "old-key-from-disk",
            "activated_at": "2026-01-01T00:00:00Z"
        }"#;
        let state: LicenseState = serde_json::from_str(json).unwrap();
        assert_eq!(state.key, Some("old-key-from-disk".into()));
        assert_eq!(state.status, LicenseStatus::Active);
    }

    #[test]
    fn key_field_defaults_to_none_when_absent() {
        let json = r#"{"status": "trial"}"#;
        let state: LicenseState = serde_json::from_str(json).unwrap();
        assert!(state.key.is_none());
    }

    #[test]
    fn serialization_roundtrip_loses_key() {
        let original = LicenseState {
            status: LicenseStatus::Active,
            key: Some("secret".into()),
            ..Default::default()
        };
        let json_str = serde_json::to_string(&original).unwrap();
        let restored: LicenseState = serde_json::from_str(&json_str).unwrap();
        // key should be gone after serialize -> deserialize
        assert!(restored.key.is_none());
    }

    // ── start_trial ─────────────────────────────────────────────────

    #[test]
    fn start_trial_returns_trial_status() {
        let state = start_trial();
        assert_eq!(state.status, LicenseStatus::Trial);
    }

    #[test]
    fn start_trial_sets_trial_started_at() {
        let before = chrono::Utc::now();
        let state = start_trial();
        let after = chrono::Utc::now();

        let started = state.trial_started_at.as_ref().unwrap();
        let ts = chrono::DateTime::parse_from_rfc3339(started)
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert!(ts >= before && ts <= after);
    }

    #[test]
    fn start_trial_other_fields_none() {
        let state = start_trial();
        assert!(state.key.is_none());
        assert!(state.activated_at.is_none());
        assert!(state.validated_at.is_none());
        assert!(state.activation_id.is_none());
    }

    // ── trial_days_remaining ────────────────────────────────────────

    #[test]
    fn fresh_trial_has_14_days() {
        let state = state_with_trial_started(0);
        assert_eq!(trial_days_remaining(&state), TRIAL_DAYS);
    }

    #[test]
    fn trial_started_7_days_ago_has_7_days() {
        let state = state_with_trial_started(7);
        assert_eq!(trial_days_remaining(&state), 7);
    }

    #[test]
    fn trial_started_13_days_ago_has_1_day() {
        let state = state_with_trial_started(13);
        assert_eq!(trial_days_remaining(&state), 1);
    }

    #[test]
    fn trial_started_14_days_ago_returns_zero() {
        let state = state_with_trial_started(14);
        assert_eq!(trial_days_remaining(&state), 0);
    }

    #[test]
    fn trial_started_30_days_ago_never_negative() {
        let state = state_with_trial_started(30);
        assert_eq!(trial_days_remaining(&state), 0);
    }

    #[test]
    fn trial_days_remaining_no_timestamp_returns_zero() {
        let state = LicenseState {
            status: LicenseStatus::Trial,
            trial_started_at: None,
            ..Default::default()
        };
        assert_eq!(trial_days_remaining(&state), 0);
    }

    #[test]
    fn trial_days_remaining_invalid_timestamp_returns_zero() {
        let state = LicenseState {
            status: LicenseStatus::Trial,
            trial_started_at: Some("not-a-date".into()),
            ..Default::default()
        };
        assert_eq!(trial_days_remaining(&state), 0);
    }

    // ── check_trial (via check_license_state) ───────────────────────

    #[test]
    fn check_trial_within_period_returns_trial() {
        let state = state_with_trial_started(5);
        assert_eq!(check_license_state(&state), LicenseStatus::Trial);
    }

    #[test]
    fn check_trial_at_day_13_returns_trial() {
        let state = state_with_trial_started(13);
        assert_eq!(check_license_state(&state), LicenseStatus::Trial);
    }

    #[test]
    fn check_trial_expired_returns_trial_expired() {
        let state = state_with_trial_started(14);
        assert_eq!(check_license_state(&state), LicenseStatus::TrialExpired);
    }

    #[test]
    fn check_trial_well_past_expiry_returns_trial_expired() {
        let state = state_with_trial_started(100);
        assert_eq!(check_license_state(&state), LicenseStatus::TrialExpired);
    }

    // ── check_license_state ─────────────────────────────────────────

    #[test]
    fn active_with_recent_validation_stays_active() {
        let state = active_state_validated_days_ago(1);
        assert_eq!(check_license_state(&state), LicenseStatus::Active);
    }

    #[test]
    fn active_within_grace_period_stays_active() {
        let state = active_state_validated_days_ago(OFFLINE_GRACE_DAYS);
        assert_eq!(check_license_state(&state), LicenseStatus::Active);
    }

    #[test]
    fn active_past_grace_period_returns_invalid() {
        let state = active_state_validated_days_ago(OFFLINE_GRACE_DAYS + 1);
        assert_eq!(check_license_state(&state), LicenseStatus::Invalid);
    }

    #[test]
    fn active_way_past_grace_returns_invalid() {
        let state = active_state_validated_days_ago(30);
        assert_eq!(check_license_state(&state), LicenseStatus::Invalid);
    }

    #[test]
    fn active_with_no_validated_at_stays_active() {
        let state = LicenseState {
            status: LicenseStatus::Active,
            validated_at: None,
            ..Default::default()
        };
        assert_eq!(check_license_state(&state), LicenseStatus::Active);
    }

    #[test]
    fn active_with_invalid_validated_at_stays_active() {
        // Bad timestamp can't be parsed, so grace-period check is skipped
        let state = LicenseState {
            status: LicenseStatus::Active,
            validated_at: Some("garbage".into()),
            ..Default::default()
        };
        assert_eq!(check_license_state(&state), LicenseStatus::Active);
    }

    #[test]
    fn unknown_returns_unknown() {
        let state = LicenseState::default();
        assert_eq!(check_license_state(&state), LicenseStatus::Unknown);
    }

    #[test]
    fn invalid_returns_invalid() {
        let state = LicenseState {
            status: LicenseStatus::Invalid,
            ..Default::default()
        };
        assert_eq!(check_license_state(&state), LicenseStatus::Invalid);
    }

    #[test]
    fn trial_expired_returns_trial_expired() {
        let state = LicenseState {
            status: LicenseStatus::TrialExpired,
            ..Default::default()
        };
        assert_eq!(check_license_state(&state), LicenseStatus::TrialExpired);
    }

    // ── check_license_state_cached ──────────────────────────────────

    #[test]
    fn cached_returns_cached_result_within_ttl() {
        let cache = ValidationCacheState::default();
        // Pre-populate cache with Active result
        {
            let mut guard = cache.0.lock().unwrap();
            guard.last_checked = Some(chrono::Utc::now());
            guard.last_result = Some(LicenseStatus::Active);
        }
        let state = active_state_validated_days_ago(1);
        let result = check_license_state_cached(&state, &cache, None);
        assert_eq!(result, LicenseStatus::Active);
    }

    #[test]
    fn cached_returns_stale_cache_ignored() {
        let cache = ValidationCacheState::default();
        // Pre-populate cache with old result
        {
            let mut guard = cache.0.lock().unwrap();
            guard.last_checked =
                Some(chrono::Utc::now() - chrono::Duration::seconds(VALIDATION_CACHE_SECS + 1));
            guard.last_result = Some(LicenseStatus::Active);
        }
        // State is active and within grace period — should bypass stale cache and return Active
        let state = active_state_validated_days_ago(1);
        let result = check_license_state_cached(&state, &cache, None);
        assert_eq!(result, LicenseStatus::Active);
    }

    #[test]
    fn cached_active_within_grace_no_revalidation() {
        let cache = ValidationCacheState::default();
        // Empty cache, but state is within grace period — no validation needed
        let state = active_state_validated_days_ago(3);
        let result = check_license_state_cached(&state, &cache, None);
        assert_eq!(result, LicenseStatus::Active);
    }

    #[test]
    fn cached_active_past_grace_no_key_returns_invalid() {
        let cache = ValidationCacheState::default();
        let state = active_state_validated_days_ago(OFFLINE_GRACE_DAYS + 1);
        // No key available and grace period expired — should return Invalid
        let result = check_license_state_cached(&state, &cache, None);
        assert_eq!(result, LicenseStatus::Invalid);
    }

    #[test]
    fn cached_trial_delegates_to_check_trial() {
        let cache = ValidationCacheState::default();
        let state = state_with_trial_started(5);
        let result = check_license_state_cached(&state, &cache, None);
        assert_eq!(result, LicenseStatus::Trial);
    }

    #[test]
    fn cached_trial_expired_delegates_to_check_trial() {
        let cache = ValidationCacheState::default();
        let state = state_with_trial_started(20);
        let result = check_license_state_cached(&state, &cache, None);
        assert_eq!(result, LicenseStatus::TrialExpired);
    }

    #[test]
    fn cached_unknown_passes_through() {
        let cache = ValidationCacheState::default();
        let state = LicenseState::default();
        let result = check_license_state_cached(&state, &cache, None);
        assert_eq!(result, LicenseStatus::Unknown);
    }

    #[test]
    fn cached_invalid_passes_through() {
        let cache = ValidationCacheState::default();
        let state = LicenseState {
            status: LicenseStatus::Invalid,
            ..Default::default()
        };
        let result = check_license_state_cached(&state, &cache, None);
        assert_eq!(result, LicenseStatus::Invalid);
    }

    // ── LicenseStatus serde ─────────────────────────────────────────

    #[test]
    fn license_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&LicenseStatus::TrialExpired).unwrap(),
            "\"trial_expired\""
        );
        assert_eq!(
            serde_json::to_string(&LicenseStatus::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&LicenseStatus::Unknown).unwrap(),
            "\"unknown\""
        );
    }

    #[test]
    fn license_status_deserializes_snake_case() {
        let status: LicenseStatus = serde_json::from_str("\"trial_expired\"").unwrap();
        assert_eq!(status, LicenseStatus::TrialExpired);
    }

    // ── ValidationCache ─────────────────────────────────────────────

    #[test]
    fn validation_cache_default_is_empty() {
        let cache = ValidationCache::default();
        assert!(cache.last_checked.is_none());
        assert!(cache.last_result.is_none());
    }

    #[test]
    fn validation_cache_state_default_creates_unlocked_mutex() {
        let cache_state = ValidationCacheState::default();
        let guard = cache_state.0.lock().unwrap();
        assert!(guard.last_checked.is_none());
    }
}
