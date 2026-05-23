//! End-to-end integration test for the live watcher. Plugs an `mpsc::Sender`
//! into the `WatchManager` (in place of the production `tauri::ipc::Channel`)
//! so we can capture emitted events without standing up tauri.
//!
//! These tests touch the real filesystem and depend on notify-debouncer-full
//! firing within ~2 seconds of a Create. Flaky on heavily loaded CI machines;
//! if that becomes a problem, increase the recv timeout.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use image::{ImageFormat, Rgb, RgbImage};
use pixmill_core::WatchedFolder;
use pixmill_lib::watch::{EventSink, WatchEvent, WatchFolderStatus, WatchManager};

struct MpscSink(mpsc::Sender<WatchEvent>);
impl EventSink for MpscSink {
    fn send(&self, event: WatchEvent) {
        let _ = self.0.send(event);
    }
}

fn fresh_tempdir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pixmill-watch-it-{}-{}-{}",
        std::process::id(),
        label,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_red_jpeg(dir: &PathBuf, name: &str, w: u32, h: u32) -> PathBuf {
    let mut img = RgbImage::new(w, h);
    for p in img.pixels_mut() {
        *p = Rgb([220, 50, 50]);
    }
    let path = dir.join(name);
    img.save_with_format(&path, ImageFormat::Jpeg).unwrap();
    path
}

fn recv_first_file_added(
    rx: &mpsc::Receiver<WatchEvent>,
    timeout: Duration,
) -> Option<(String, String)> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(WatchEvent::FileAdded { folder, item }) => {
                return Some((folder, item.path.to_string_lossy().into_owned()));
            }
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
    None
}

#[test]
fn watcher_emits_file_added_for_new_jpeg() {
    let dir = fresh_tempdir("added");
    let (tx, rx) = mpsc::channel::<WatchEvent>();

    let mut mgr = WatchManager::new();
    mgr.set_sink(Box::new(MpscSink(tx)));
    mgr.add(
        WatchedFolder {
            path: dir.clone(),
            recursive: true,
            auto_process: false,
        },
        None,
    )
    .unwrap();
    mgr.ensure_running().expect("boot debouncer");

    // Give notify a beat to install the OS-level watch.
    std::thread::sleep(Duration::from_millis(200));

    let written = write_red_jpeg(&dir, "fresh.jpg", 16, 16);
    let observed = recv_first_file_added(&rx, Duration::from_secs(2)).expect("FileAdded within 2s");
    assert_eq!(PathBuf::from(observed.0), dir);
    assert_eq!(PathBuf::from(observed.1), written);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn watcher_emits_file_added_on_data_modify() {
    // Phase 4: in-place edits (Photoshop save, ExifTool, screenshot re-save)
    // must re-emit `FileAdded` so the JS handler can flip the queue row back
    // to `pending` and (under auto-process) re-process it.
    let dir = fresh_tempdir("modify");
    let (tx, rx) = mpsc::channel::<WatchEvent>();

    let mut mgr = WatchManager::new();
    mgr.set_sink(Box::new(MpscSink(tx)));
    mgr.add(
        WatchedFolder {
            path: dir.clone(),
            recursive: true,
            auto_process: false,
        },
        None,
    )
    .unwrap();
    mgr.ensure_running().unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // First write is a Create — expect one FileAdded.
    let path = write_red_jpeg(&dir, "edit.jpg", 16, 16);
    let first = recv_first_file_added(&rx, Duration::from_secs(2)).expect("initial FileAdded");
    assert_eq!(PathBuf::from(first.1), path);

    // Overwrite the file in place with different bytes. Phase 4 promotes
    // `Modify(Data)` into a FileAdded for the same path.
    let mut img = RgbImage::new(16, 16);
    for p in img.pixels_mut() {
        *p = Rgb([50, 220, 50]);
    }
    img.save_with_format(&path, ImageFormat::Jpeg).unwrap();

    let second = recv_first_file_added(&rx, Duration::from_secs(2))
        .expect("Phase 4 should re-emit FileAdded on Modify(Data)");
    assert_eq!(PathBuf::from(second.1), path);

    let _ = std::fs::remove_dir_all(&dir);
}

fn recv_status(rx: &mpsc::Receiver<WatchEvent>, timeout: Duration) -> Option<WatchFolderStatus> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(WatchEvent::FolderStatus { status, .. }) => return Some(status),
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
    None
}

#[test]
fn retry_watched_folder_attaches_after_creation() {
    // Phase 4: register a path that doesn't exist yet → expect Error status.
    // Create the dir, drop a JPEG, then `retry_folder` and expect Watching +
    // FileAdded.
    let parent = std::env::temp_dir().join(format!(
        "pixmill-retry-it-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dir = parent.join("not-yet");
    // Ensure the path does not exist.
    let _ = std::fs::remove_dir_all(&parent);
    let (tx, rx) = mpsc::channel::<WatchEvent>();

    let mut mgr = WatchManager::new();
    mgr.set_sink(Box::new(MpscSink(tx)));
    mgr.add(
        WatchedFolder {
            path: dir.clone(),
            recursive: true,
            auto_process: false,
        },
        None,
    )
    .unwrap();
    // First `ensure_running` boot attempt should land an Error status.
    mgr.ensure_running().unwrap();

    let status = recv_status(&rx, Duration::from_secs(2)).expect("initial FolderStatus");
    assert!(
        matches!(status, WatchFolderStatus::Error { .. }),
        "expected Error status for missing path, got {status:?}"
    );

    // Now create the directory and retry — should flip to Watching.
    std::fs::create_dir_all(&dir).unwrap();
    mgr.retry_folder(&dir);

    let status = recv_status(&rx, Duration::from_secs(2)).expect("retry FolderStatus");
    assert!(
        matches!(status, WatchFolderStatus::Watching),
        "expected Watching after retry, got {status:?}"
    );

    // And a Create should now reach us.
    std::thread::sleep(Duration::from_millis(200));
    let written = write_red_jpeg(&dir, "after.jpg", 16, 16);
    let observed =
        recv_first_file_added(&rx, Duration::from_secs(2)).expect("FileAdded after retry");
    assert_eq!(PathBuf::from(observed.1), written);

    let _ = std::fs::remove_dir_all(&parent);
}
