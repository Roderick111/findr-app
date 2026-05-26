use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::process::Command;

const POLAR_ORG_ID: &str = "499639ab-c131-4dc7-9fe7-a4cde74f56f4";
const TRIAL_DAYS: i64 = 14;
const OFFLINE_GRACE_DAYS: i64 = 7;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LicenseState {
    pub status: LicenseStatus,
    pub key: Option<String>,
    pub activated_at: Option<String>,
    pub validated_at: Option<String>,
    pub activation_id: Option<String>,
    pub trial_started_at: Option<String>,
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

pub fn activate_license(key: &str) -> Result<LicenseState, String> {
    let fingerprint = machine_fingerprint().map_err(|e| format!("fingerprint failed: {e}"))?;
    let label = format!("findr-desktop-{}", &fingerprint[..8]);

    let resp: ureq::Response = ureq::post(
        "https://api.polar.sh/v1/customer-portal/license-keys/activate",
    )
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
        key: Some(key.to_string()),
        activated_at: Some(now.clone()),
        validated_at: Some(now),
        activation_id: Some(parsed.id),
        trial_started_at: None,
    })
}

pub fn validate_license(key: &str, activation_id: &str) -> Result<LicenseStatus, String> {
    let resp: ureq::Response = ureq::post(
        "https://api.polar.sh/v1/customer-portal/license-keys/validate",
    )
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

pub fn check_license_state(state: &LicenseState) -> LicenseStatus {
    match &state.status {
        LicenseStatus::Active => {
            if let Some(validated_at) = &state.validated_at {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(validated_at) {
                    let age = chrono::Utc::now() - ts.with_timezone(&chrono::Utc);
                    if age.num_days() > OFFLINE_GRACE_DAYS {
                        if let (Some(key), Some(aid)) = (&state.key, &state.activation_id) {
                            return validate_license(key, aid).unwrap_or(LicenseStatus::Active);
                        }
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
