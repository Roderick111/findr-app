use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const SYNC_INTERVAL: Duration = Duration::from_secs(300);
const MAX_BACKOFF: Duration = Duration::from_secs(600); // 10 minutes
const SHUTDOWN_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Shared lock to prevent daemon and user-triggered sync from racing.
/// Stored in Tauri managed state.
pub struct SyncLock(pub AtomicBool);

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
            match rt.block_on(crate::findr_client::index_status(&app)) {
                Ok(status) => {
                    if status.files_indexed.unwrap_or(0) == 0 {
                        let _ = app.emit("index-progress", "Starting initial sync...");
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
                                let _ = app.emit("index-progress", "Initial sync complete");
                            }
                            Err(e) => {
                                eprintln!("[daemon] initial sync failed: {e}");
                                let _ =
                                    app.emit("index-progress", format!("Initial sync error: {e}"));
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[daemon] index status check failed: {e}");
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

            let _ = app.emit("index-sync", "syncing");
            let result = rt.block_on(crate::findr_client::sync(&app));

            // Release sync lock
            if let Some(ref lock) = sync_lock {
                lock.release();
            }

            match result {
                Ok(_) => {
                    consecutive_failures = 0;
                    let _ = app.emit("index-sync", "complete");
                }
                Err(e) => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    eprintln!(
                        "[daemon] sync failed (attempt {}): {e}",
                        consecutive_failures
                    );
                    let _ = app.emit("index-sync", format!("error: {e}"));
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
}
