use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const SYNC_INTERVAL: Duration = Duration::from_secs(300);

pub fn spawn_index_daemon(app: AppHandle) {
    std::thread::spawn(move || {
        let rt = tauri::async_runtime::handle();

        if let Ok(status) = rt.block_on(crate::findr_client::index_status(&app)) {
            if status.files_indexed.unwrap_or(0) == 0 {
                let _ = app.emit("index-progress", "Starting initial sync...");
                let _ = rt.block_on(crate::findr_client::sync(&app));
                let _ = app.emit("index-progress", "Initial sync complete");
            }
        }

        let mut last_sync = Instant::now();

        loop {
            std::thread::sleep(Duration::from_secs(5));

            if last_sync.elapsed() >= SYNC_INTERVAL {
                let _ = app.emit("index-sync", "syncing");
                match rt.block_on(crate::findr_client::sync(&app)) {
                    Ok(_) => {
                        let _ = app.emit("index-sync", "complete");
                    }
                    Err(e) => {
                        let _ = app.emit("index-sync", format!("error: {}", e));
                    }
                }
                last_sync = Instant::now();
            }
        }
    });
}
