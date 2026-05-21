mod commands;
mod persistence;
pub mod watch;

use std::sync::Mutex;
use std::time::Duration;

use tauri::Manager;

use watch::WatchManager;

/// How often the background task re-attempts to attach watches for folders
/// stuck in `Error` status (e.g. external drive that was unplugged). The
/// per-row "Retry now" button reuses the same code path on demand.
const RETRY_POLL_INTERVAL: Duration = Duration::from_secs(30);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(WatchManager::new()))
        .setup(|app| {
            // Seed the manager with persisted watched folders so a later
            // `subscribe_watch_events` can attach them all in one go.
            // Failures are logged but non-fatal — the user can re-add a
            // folder if its row never shows up.
            let handle = app.handle().clone();
            if let Ok(Some(state)) = persistence::load(&handle) {
                if let Some(mgr) = handle.try_state::<Mutex<WatchManager>>() {
                    if let Ok(mut mgr) = mgr.lock() {
                        mgr.seed(state.watched_folders);
                    }
                }
            }
            // Background retry task for folders stuck in Error status. We
            // acquire the lock per-folder rather than across the whole tick
            // so user commands (add/remove/set) only ever wait for one
            // syscall, not the full iteration.
            let retry_handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(RETRY_POLL_INTERVAL);
                let to_retry: Vec<pixmill_core::WatchedFolder> = match retry_handle
                    .try_state::<Mutex<WatchManager>>()
                {
                    Some(state) => match state.lock() {
                        Ok(mgr) => mgr.error_folders(),
                        Err(_) => continue,
                    },
                    None => continue,
                };
                for folder in to_retry {
                    if let Some(state) = retry_handle.try_state::<Mutex<WatchManager>>() {
                        if let Ok(mut mgr) = state.lock() {
                            mgr.retry_folder(&folder.path);
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ingest_paths,
            commands::read_metadata,
            commands::make_thumbnail,
            commands::run_batch,
            commands::preview_one,
            commands::load_source,
            commands::load_settings,
            commands::save_settings,
            commands::add_watched_folder,
            commands::remove_watched_folder,
            commands::set_watched_folder_config,
            commands::validate_output_dir,
            commands::subscribe_watch_events,
            commands::retry_watched_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
