use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const SYNC_INTERVAL: Duration = Duration::from_secs(300);
const MAX_BACKOFF: Duration = Duration::from_secs(600); // 10 minutes
const SHUTDOWN_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Shared lock to prevent daemon and user-triggered sync from racing.
/// Stored in Tauri managed state.
pub struct SyncLock(pub AtomicBool);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IndexActivity {
    pub phase: String,
    pub message: String,
    pub active: bool,
}

#[derive(Debug)]
pub struct IndexActivityState(Mutex<IndexActivity>);

impl Default for IndexActivityState {
    fn default() -> Self {
        Self(Mutex::new(IndexActivity {
            phase: "checking".into(),
            message: "Checking search index…".into(),
            active: true,
        }))
    }
}

impl IndexActivityState {
    pub fn snapshot(&self) -> IndexActivity {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn update(&self, app: &AppHandle, phase: &str, message: &str, active: bool) {
        let activity = IndexActivity {
            phase: phase.into(),
            message: message.into(),
            active,
        };
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = activity.clone();
        let _ = app.emit("index-activity", activity);
    }
}

impl Default for SyncLock {
    fn default() -> Self {
        Self(AtomicBool::new(false))
    }
}

impl SyncLock {
    /// Try to acquire the lock. Returns true if acquired.
    pub fn try_acquire(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Release the lock.
    pub fn release(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn rebuild_index(app: &AppHandle) -> Result<String, String> {
    let lock = app.try_state::<SyncLock>();
    if let Some(ref lock) = lock {
        if !lock.try_acquire() {
            return Err("sync already in progress".into());
        }
    }
    let result = tauri::async_runtime::block_on(crate::findr_client::rebuild(app, None, None));
    if let Some(ref lock) = lock {
        lock.release();
    }
    result
}

fn repair_corrupt_index(app: &AppHandle, activity: &IndexActivityState) -> bool {
    activity.update(app, "indexing", "Repairing search index…", true);
    match rebuild_index(app) {
        Ok(_) => {
            activity.update(app, "ready", "Search is ready.", false);
            true
        }
        Err(error) => {
            eprintln!("[daemon] index repair failed: {error}");
            activity.update(app, "error", "Search repair failed.", false);
            false
        }
    }
}

/// Spawn the background index daemon. Returns a shutdown flag that can be
/// set to `true` to cleanly stop the daemon thread.
pub fn spawn_index_daemon(app: AppHandle) -> Arc<AtomicBool> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    std::thread::spawn(move || {
        let rt = tauri::async_runtime::handle();

        // Client owns process timeout and kill semantics. Maintenance jobs have
        // a long deadline because a home-directory rebuild is not bounded to 30s.
        if !shutdown_clone.load(Ordering::SeqCst) {
            let activity = app.state::<IndexActivityState>();
            match rt.block_on(crate::findr_client::doctor(&app)) {
                Ok(report)
                    if report.database.health == crate::findr_client::DatabaseHealth::Corrupt =>
                {
                    repair_corrupt_index(&app, &activity);
                }
                Ok(report)
                    if report.database.health
                        == crate::findr_client::DatabaseHealth::Unavailable =>
                {
                    eprintln!(
                        "[daemon] database unavailable: {}",
                        report.database.error.as_deref().unwrap_or("unknown error")
                    );
                    activity.update(
                        &app,
                        "error",
                        "Search index is unavailable. Check folder permissions.",
                        false,
                    );
                }
                Ok(report) if report.scan_paths.is_empty() => {
                    activity.update(
                        &app,
                        "needs_setup",
                        "Choose a folder to start searching.",
                        false,
                    );
                }
                Ok(_) => match rt.block_on(crate::findr_client::index_status(&app)) {
                    Ok(status) if status.files_indexed.unwrap_or(0) == 0 => {
                        activity.update(
                            &app,
                            "indexing",
                            "Preparing search — finding your files…",
                            true,
                        );
                        let sync_lock = app.try_state::<SyncLock>();
                        let acquired = sync_lock.as_ref().is_none_or(|lock| lock.try_acquire());
                        let result = if acquired {
                            let result = rt.block_on(crate::findr_client::sync(&app));
                            if let Some(lock) = sync_lock.as_ref() {
                                lock.release();
                            }
                            result
                        } else {
                            Err("sync already in progress".to_string())
                        };
                        match result {
                            Ok(_) => {
                                activity.update(&app, "ready", "Search is ready.", false);
                            }
                            Err(e) => {
                                eprintln!("[daemon] initial sync failed: {e}");
                                if crate::findr_client::is_corrupt_index_error(&e) {
                                    repair_corrupt_index(&app, &activity);
                                } else {
                                    activity.update(
                                        &app,
                                        "error",
                                        &format!("Search setup failed: {e}"),
                                        false,
                                    );
                                }
                            }
                        }
                    }
                    Ok(_) => activity.update(&app, "ready", "Search is ready.", false),
                    Err(e) => {
                        eprintln!("[daemon] index status check failed: {e}");
                        if crate::findr_client::is_corrupt_index_error(&e) {
                            repair_corrupt_index(&app, &activity);
                        } else {
                            activity.update(&app, "error", "Couldn’t check search status.", false);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("[daemon] index status check failed: {e}");
                    activity.update(&app, "error", "Couldn’t check search status.", false);
                }
            }
        }

        let mut consecutive_failures: u32 = 0;

        loop {
            // Sleep for full interval, checking shutdown flag periodically
            let sleep_duration = if consecutive_failures > 0 {
                // Exponential backoff: 300s * 2^failures, capped at MAX_BACKOFF
                let multiplier = 1u64.checked_shl(consecutive_failures).unwrap_or(u64::MAX);
                let backoff_secs = SYNC_INTERVAL.as_secs().saturating_mul(multiplier);
                Duration::from_secs(backoff_secs).min(MAX_BACKOFF)
            } else {
                SYNC_INTERVAL
            };

            // Interruptible sleep: check shutdown flag every SHUTDOWN_CHECK_INTERVAL
            let mut slept = Duration::ZERO;
            while slept < sleep_duration {
                if shutdown_clone.load(Ordering::SeqCst) {
                    eprintln!("[daemon] shutdown requested, exiting");
                    return;
                }
                std::thread::sleep(SHUTDOWN_CHECK_INTERVAL);
                slept += SHUTDOWN_CHECK_INTERVAL;
            }

            if shutdown_clone.load(Ordering::SeqCst) {
                eprintln!("[daemon] shutdown requested, exiting");
                return;
            }

            // Try to acquire sync lock — skip cycle if user-triggered sync is running
            let sync_lock = app.try_state::<SyncLock>();
            let acquired = match &sync_lock {
                Some(lock) => lock.try_acquire(),
                None => true, // no lock in state — proceed anyway
            };

            if !acquired {
                eprintln!("[daemon] sync lock held by user operation, skipping cycle");
                continue;
            }

            let activity = app.state::<IndexActivityState>();
            activity.update(&app, "syncing", "Updating search…", true);
            let result = rt.block_on(crate::findr_client::sync(&app));

            // Release sync lock
            if let Some(ref lock) = sync_lock {
                lock.release();
            }

            match result {
                Ok(_) => {
                    consecutive_failures = 0;
                    activity.update(&app, "ready", "Search updated.", false);
                }
                Err(e) if crate::findr_client::is_corrupt_index_error(&e) => {
                    eprintln!("[daemon] corrupt index detected: {e}");
                    if repair_corrupt_index(&app, &activity) {
                        consecutive_failures = 0;
                    } else {
                        consecutive_failures = consecutive_failures.saturating_add(1);
                    }
                }
                Err(e) => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    eprintln!(
                        "[daemon] sync failed (attempt {}): {e}",
                        consecutive_failures
                    );
                    activity.update(&app, "error", "Search update failed.", false);
                }
            }
        }
    });

    shutdown
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SyncLock ────────────────────────────────────────────────────

    #[test]
    fn sync_lock_default_is_unlocked() {
        let lock = SyncLock::default();
        assert!(!lock.0.load(Ordering::SeqCst));
    }

    #[test]
    fn try_acquire_succeeds_when_unlocked() {
        let lock = SyncLock::default();
        assert!(lock.try_acquire());
    }

    #[test]
    fn try_acquire_fails_when_already_held() {
        let lock = SyncLock::default();
        assert!(lock.try_acquire());
        assert!(!lock.try_acquire()); // second acquire fails
    }

    #[test]
    fn release_allows_reacquire() {
        let lock = SyncLock::default();
        assert!(lock.try_acquire());
        lock.release();
        assert!(lock.try_acquire()); // can acquire again after release
    }

    #[test]
    fn release_when_not_held_is_safe() {
        let lock = SyncLock::default();
        lock.release(); // no-op, should not panic
        assert!(lock.try_acquire());
    }

    #[test]
    fn acquire_release_cycle_multiple_times() {
        let lock = SyncLock::default();
        for _ in 0..10 {
            assert!(lock.try_acquire());
            lock.release();
        }
    }

    // ── Constants sanity checks ─────────────────────────────────────

    #[test]
    fn sync_interval_is_5_minutes() {
        assert_eq!(SYNC_INTERVAL.as_secs(), 300);
    }

    #[test]
    fn max_backoff_is_10_minutes() {
        assert_eq!(MAX_BACKOFF.as_secs(), 600);
    }

    #[test]
    fn max_backoff_exceeds_sync_interval() {
        assert!(MAX_BACKOFF > SYNC_INTERVAL);
    }

    #[test]
    fn index_activity_starts_in_checking_state() {
        let activity = IndexActivityState::default().snapshot();
        assert_eq!(activity.phase, "checking");
        assert_eq!(activity.message, "Checking search index…");
        assert!(activity.active);
    }
}
