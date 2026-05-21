use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use notify_debouncer_full::{
    new_debouncer,
    notify::{
        event::{ModifyKind, RenameMode},
        EventKind, RecommendedWatcher, RecursiveMode,
    },
    DebounceEventResult, Debouncer, RecommendedCache,
};
use pixmill_core::{formats::is_supported_input, metadata, ImageMetadata, WatchedFolder};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

// ─── Wire types ────────────────────────────────────────────────────────────

/// Live status of one registered folder. Surfaces to the UI as a badge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum WatchFolderStatus {
    Watching,
    Paused,
    Error { message: String },
}

/// Streaming event sent from the watcher thread to the JS handler.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum WatchEvent {
    FileAdded {
        folder: String,
        item: ImageMetadata,
    },
    FileRemoved {
        folder: String,
        path: String,
    },
    FolderStatus {
        folder: String,
        status: WatchFolderStatus,
    },
}

/// Reasons `WatchManager::add` may reject a folder. We surface these as plain
/// strings to JS through the tauri command layer.
#[derive(Debug)]
pub enum WatchError {
    OverlapsOutputDir {
        folder: PathBuf,
        output_dir: PathBuf,
    },
    OverlapsExistingFolder {
        new: PathBuf,
        existing: PathBuf,
    },
    AutoProcessRequiresOutputDir,
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::OverlapsOutputDir { folder, output_dir } => write!(
                f,
                "watch folder '{}' overlaps the output directory '{}'",
                folder.display(),
                output_dir.display()
            ),
            WatchError::OverlapsExistingFolder { new, existing } => write!(
                f,
                "watch folder '{}' overlaps an existing watch folder '{}'",
                new.display(),
                existing.display()
            ),
            WatchError::AutoProcessRequiresOutputDir => {
                write!(f, "auto-process requires an output folder to be set first")
            }
        }
    }
}

impl std::error::Error for WatchError {}

// ─── Path helpers ──────────────────────────────────────────────────────────

/// True iff `a` is an ancestor of (or equal to) `b`, or vice versa. Used to
/// reject configurations where the output dir lives inside a watched folder
/// (which would cause an infinite processing loop) and vice versa.
pub fn is_path_overlap(a: &Path, b: &Path) -> bool {
    let ca = a.canonicalize();
    let cb = b.canonicalize();
    let (a_norm, b_norm) = match (ca, cb) {
        (Ok(ca), Ok(cb)) => (ca, cb),
        _ => (normalize_lexical(a), normalize_lexical(b)),
    };
    a_norm == b_norm || a_norm.starts_with(&b_norm) || b_norm.starts_with(&a_norm)
}

/// Lexical normalization: collapse `.` and `..` segments where we can without
/// touching the filesystem. Best-effort fallback when `canonicalize` fails
/// (e.g. the file doesn't exist yet).
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        use std::path::Component;
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

// ─── Event sink abstraction ────────────────────────────────────────────────

/// Sink for `WatchEvent`s. Production wraps a `tauri::ipc::Channel`; tests
/// substitute an `mpsc::Sender` so we don't have to stand up tauri.
pub trait EventSink: Send + 'static {
    fn send(&self, event: WatchEvent);
}

struct ChannelSink(Channel<WatchEvent>);
impl EventSink for ChannelSink {
    fn send(&self, event: WatchEvent) {
        let _ = self.0.send(event);
    }
}

// ─── Manager ───────────────────────────────────────────────────────────────

/// Shared state visible to both the WatchManager (mutating side) and the
/// dispatcher thread (read side: per-event path → folder lookup).
type SharedRegistry = Arc<Mutex<HashMap<PathBuf, WatchedFolder>>>;
type SharedSink = Arc<Mutex<Option<Box<dyn EventSink>>>>;

pub struct WatchManager {
    /// Live registry, shared with the dispatcher thread.
    registered: SharedRegistry,
    /// Current event sink. Replaced on every `subscribe`. Long-lived;
    /// dispatcher thread holds a clone and keeps emitting through it.
    sink: SharedSink,
    /// Lazily constructed on first `subscribe`. Owns the OS-level watches;
    /// dropping it stops all of them.
    debouncer: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
    /// Latest known status per folder, kept in sync with the events we emit.
    /// Phase 4 reads this to decide which folders the polling retry should
    /// re-attach.
    pub folder_status: HashMap<PathBuf, WatchFolderStatus>,
}

impl WatchManager {
    pub fn new() -> Self {
        Self {
            registered: Arc::new(Mutex::new(HashMap::new())),
            sink: Arc::new(Mutex::new(None)),
            debouncer: None,
            folder_status: HashMap::new(),
        }
    }

    /// All currently registered folders. Hands back clones — fine for the
    /// "few dozen folders" scale this UI is designed for.
    pub fn registered_folders(&self) -> Vec<WatchedFolder> {
        self.registered.lock().unwrap().values().cloned().collect()
    }

    /// Folders whose latest emitted status is `Error`. The Phase 4 polling
    /// retry task snapshots this list each tick and re-attaches each one.
    pub fn error_folders(&self) -> Vec<WatchedFolder> {
        let registry = self.registered.lock().unwrap();
        self.folder_status
            .iter()
            .filter(|(_, status)| matches!(status, WatchFolderStatus::Error { .. }))
            .filter_map(|(path, _)| registry.get(path).cloned())
            .collect()
    }

    /// Manual retry for one folder (used by the "Retry now" button and the
    /// polling task). Reuses `try_attach_watch`; success/failure lands on the
    /// row via a fresh `FolderStatus` event rather than the return value.
    pub fn retry_folder(&mut self, path: &Path) {
        let folder = self.registered.lock().unwrap().get(path).cloned();
        if let Some(folder) = folder {
            self.try_attach_watch(&folder);
        }
    }

    /// Bypass-validation seeding from persisted state at app boot. The
    /// persisted list was already validated when the user added each folder,
    /// so re-running overlap checks would be redundant (and risk dropping
    /// folders if the persisted state somehow diverged). The OS-level watch
    /// itself isn't attached here — that happens on the first `subscribe`.
    pub fn seed(&mut self, folders: Vec<WatchedFolder>) {
        let mut registry = self.registered.lock().unwrap();
        for folder in folders {
            self.folder_status
                .insert(folder.path.clone(), WatchFolderStatus::Watching);
            registry.insert(folder.path.clone(), folder);
        }
    }

    pub fn add(
        &mut self,
        folder: WatchedFolder,
        output_dir: Option<&Path>,
    ) -> Result<(), WatchError> {
        if folder.auto_process && output_dir.is_none() {
            return Err(WatchError::AutoProcessRequiresOutputDir);
        }
        if let Some(out) = output_dir {
            if is_path_overlap(&folder.path, out) {
                return Err(WatchError::OverlapsOutputDir {
                    folder: folder.path.clone(),
                    output_dir: out.to_path_buf(),
                });
            }
        }
        {
            let registry = self.registered.lock().unwrap();
            for existing in registry.values() {
                if existing.path == folder.path {
                    continue;
                }
                if is_path_overlap(&folder.path, &existing.path) {
                    return Err(WatchError::OverlapsExistingFolder {
                        new: folder.path.clone(),
                        existing: existing.path.clone(),
                    });
                }
            }
        }
        self.registered
            .lock()
            .unwrap()
            .insert(folder.path.clone(), folder.clone());
        // If the debouncer is already up, attach the new watch live.
        // Otherwise the next `subscribe` call will pick it up.
        if self.debouncer.is_some() {
            self.try_attach_watch(&folder);
        } else {
            self.folder_status
                .insert(folder.path.clone(), WatchFolderStatus::Watching);
        }
        Ok(())
    }

    pub fn remove(&mut self, path: &Path) {
        self.registered.lock().unwrap().remove(path);
        self.folder_status.remove(path);
        if let Some(debouncer) = self.debouncer.as_mut() {
            // `unwatch` errors when the path wasn't registered, which is
            // fine — happens when an attach failed earlier.
            let _ = debouncer.unwatch(path);
        }
    }

    pub fn set_config(
        &mut self,
        path: &Path,
        recursive: bool,
        auto_process: bool,
        output_dir: Option<&Path>,
    ) -> Result<(), WatchError> {
        if auto_process && output_dir.is_none() {
            return Err(WatchError::AutoProcessRequiresOutputDir);
        }
        let recursive_changed = {
            let mut registry = self.registered.lock().unwrap();
            let folder = match registry.get_mut(path) {
                Some(f) => f,
                None => return Ok(()),
            };
            let changed = folder.recursive != recursive;
            folder.recursive = recursive;
            folder.auto_process = auto_process;
            changed
        };
        // Re-watch if recursive mode flipped; auto-process is purely JS-side.
        if recursive_changed {
            let folder = self
                .registered
                .lock()
                .unwrap()
                .get(path)
                .cloned();
            if let Some(folder) = folder {
                if let Some(debouncer) = self.debouncer.as_mut() {
                    let _ = debouncer.unwatch(path);
                }
                self.try_attach_watch(&folder);
            }
        }
        Ok(())
    }

    pub fn validate_output_dir(&self, output_dir: &Path) -> Result<(), WatchError> {
        let registry = self.registered.lock().unwrap();
        for existing in registry.values() {
            if is_path_overlap(&existing.path, output_dir) {
                return Err(WatchError::OverlapsOutputDir {
                    folder: existing.path.clone(),
                    output_dir: output_dir.to_path_buf(),
                });
            }
        }
        Ok(())
    }

    /// Install or replace the streaming channel. First call also boots the
    /// debouncer; subsequent calls just swap the sink — the debouncer keeps
    /// running.
    pub fn subscribe(&mut self, channel: Channel<WatchEvent>) -> Result<(), String> {
        self.set_sink(Box::new(ChannelSink(channel)));
        self.ensure_running()
    }

    /// Replace the active sink. Used by `subscribe` (production) and tests.
    pub fn set_sink(&mut self, sink: Box<dyn EventSink>) {
        *self.sink.lock().unwrap() = Some(sink);
    }

    /// Boot the debouncer + dispatcher thread on first call, then (re)attach
    /// every persisted folder. Idempotent.
    pub fn ensure_running(&mut self) -> Result<(), String> {
        if self.debouncer.is_none() {
            self.start_debouncer()?;
        }
        let folders = self.registered_folders();
        for folder in folders {
            self.try_attach_watch(&folder);
        }
        Ok(())
    }

    fn start_debouncer(&mut self) -> Result<(), String> {
        let (raw_tx, raw_rx) = mpsc::channel::<DebounceEventResult>();
        let debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: DebounceEventResult| {
                let _ = raw_tx.send(result);
            },
        )
        .map_err(|e| format!("failed to start file watcher: {e}"))?;
        let sink = Arc::clone(&self.sink);
        let registry = Arc::clone(&self.registered);
        std::thread::spawn(move || dispatcher_loop(raw_rx, sink, registry));
        self.debouncer = Some(debouncer);
        Ok(())
    }

    /// Attempt the OS-level watch for one folder; update `folder_status`
    /// and emit a `FolderStatus` event with the result. Phase 4's retry
    /// polling reuses this through `retry_watched_folder`.
    pub fn try_attach_watch(&mut self, folder: &WatchedFolder) {
        let mode = if folder.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        let status = match self.debouncer.as_mut() {
            None => WatchFolderStatus::Watching,
            Some(d) => match d.watch(&folder.path, mode) {
                Ok(()) => {
                    eprintln!(
                        "[watch] registered {} (recursive={}, auto_process={})",
                        folder.path.display(),
                        folder.recursive,
                        folder.auto_process
                    );
                    WatchFolderStatus::Watching
                }
                Err(e) => {
                    let msg = humanize_notify_error(&e);
                    eprintln!(
                        "[watch] notify error for {}: {msg}",
                        folder.path.display()
                    );
                    WatchFolderStatus::Error { message: msg }
                }
            },
        };
        self.folder_status
            .insert(folder.path.clone(), status.clone());
        self.emit(WatchEvent::FolderStatus {
            folder: folder.path.to_string_lossy().into_owned(),
            status,
        });
    }

    fn emit(&self, event: WatchEvent) {
        if let Ok(guard) = self.sink.lock() {
            if let Some(sink) = guard.as_ref() {
                sink.send(event);
            }
        }
    }
}

impl Default for WatchManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Event processing ──────────────────────────────────────────────────────

fn humanize_notify_error(err: &notify_debouncer_full::notify::Error) -> String {
    use notify_debouncer_full::notify::ErrorKind;
    match &err.kind {
        ErrorKind::PathNotFound => "folder not found".to_string(),
        ErrorKind::WatchNotFound => "watch was not previously registered".to_string(),
        ErrorKind::MaxFilesWatch => {
            "too many subdirectories to watch (Linux inotify limit reached)".to_string()
        }
        ErrorKind::Io(io_err) => format!("I/O error: {io_err}"),
        ErrorKind::Generic(s) => s.clone(),
        _ => format!("{err}"),
    }
}

fn dispatcher_loop(
    rx: mpsc::Receiver<DebounceEventResult>,
    sink: SharedSink,
    registry: SharedRegistry,
) {
    while let Ok(result) = rx.recv() {
        match result {
            Ok(events) => {
                for ev in events {
                    process_event(&ev.event, &sink, &registry);
                }
            }
            Err(errs) => {
                for e in errs {
                    eprintln!("[watch] debouncer error: {}", humanize_notify_error(&e));
                }
            }
        }
    }
    eprintln!("[watch] dispatcher loop exited; debouncer dropped");
}

fn process_event(
    event: &notify_debouncer_full::notify::Event,
    sink: &SharedSink,
    registry: &SharedRegistry,
) {
    let classified = classify_event_kind(&event.kind);
    if classified == EventClass::Ignored {
        return;
    }
    for path in &event.paths {
        if !is_supported_input(path) {
            continue;
        }
        let owner = {
            let guard = registry.lock().unwrap();
            let mut owners: Vec<PathBuf> = guard
                .keys()
                .filter(|root| path.starts_with(root))
                .cloned()
                .collect();
            if owners.len() > 1 {
                eprintln!(
                    "[watch] path {} matched multiple watched folders ({:?}); using first",
                    path.display(),
                    owners
                );
            }
            owners.pop()
        };
        let Some(owner) = owner else {
            continue;
        };
        let folder_id = owner.to_string_lossy().into_owned();
        let watch_event = match classified {
            EventClass::Added => {
                let item = metadata::read(path);
                WatchEvent::FileAdded {
                    folder: folder_id,
                    item,
                }
            }
            EventClass::Removed => WatchEvent::FileRemoved {
                folder: folder_id,
                path: path.to_string_lossy().into_owned(),
            },
            EventClass::Ignored => unreachable!(),
        };
        if let Ok(guard) = sink.lock() {
            if let Some(sink) = guard.as_ref() {
                sink.send(watch_event);
            }
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum EventClass {
    Added,
    Removed,
    Ignored,
}

fn classify_event_kind(kind: &EventKind) -> EventClass {
    match kind {
        EventKind::Create(_) => EventClass::Added,
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => EventClass::Added,
        // Phase 4: in-place edits emit `Modify(Data)`. Treat them as Added
        // so the JS handler can disambiguate by current queue status
        // (path in queue as done/error → flip back to pending).
        EventKind::Modify(ModifyKind::Data(_)) => EventClass::Added,
        EventKind::Remove(_) => EventClass::Removed,
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => EventClass::Removed,
        _ => EventClass::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(path: &str, auto: bool) -> WatchedFolder {
        WatchedFolder {
            path: PathBuf::from(path),
            recursive: true,
            auto_process: auto,
        }
    }

    // ─── is_path_overlap ─────────────────────────────────────────────────

    #[test]
    fn overlap_identical_paths() {
        let a = PathBuf::from("/tmp/a");
        assert!(is_path_overlap(&a, &a));
    }

    #[test]
    fn overlap_ancestor_descendant_both_directions() {
        let parent = PathBuf::from("/tmp/a");
        let child = PathBuf::from("/tmp/a/b");
        assert!(is_path_overlap(&parent, &child));
        assert!(is_path_overlap(&child, &parent));
    }

    #[test]
    fn overlap_siblings_and_cousins_are_disjoint() {
        let a = PathBuf::from("/tmp/a");
        let b = PathBuf::from("/tmp/b");
        assert!(!is_path_overlap(&a, &b));
        let c = PathBuf::from("/tmp/a/c");
        let d = PathBuf::from("/tmp/b/d");
        assert!(!is_path_overlap(&c, &d));
    }

    #[test]
    fn overlap_partial_segment_prefix_is_not_overlap() {
        // `/tmp/abc` is NOT inside `/tmp/ab`, despite the string prefix.
        let a = PathBuf::from("/tmp/ab");
        let b = PathBuf::from("/tmp/abc");
        assert!(!is_path_overlap(&a, &b));
    }

    #[test]
    fn overlap_handles_existing_tempdirs() {
        let parent = std::env::temp_dir().join(format!(
            "pixmill-watch-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let child = parent.join("nested");
        std::fs::create_dir_all(&child).unwrap();
        assert!(is_path_overlap(&parent, &child));
        assert!(is_path_overlap(&child, &parent));
        let _ = std::fs::remove_dir_all(&parent);
    }

    // ─── WatchManager validation ─────────────────────────────────────────

    #[test]
    fn add_rejects_auto_process_without_output_dir() {
        let mut mgr = WatchManager::new();
        let err = mgr.add(folder("/tmp/x", true), None).unwrap_err();
        assert!(matches!(err, WatchError::AutoProcessRequiresOutputDir));
    }

    #[test]
    fn add_allows_auto_process_with_output_dir() {
        let mut mgr = WatchManager::new();
        mgr.add(folder("/tmp/x", true), Some(Path::new("/tmp/out")))
            .unwrap();
    }

    #[test]
    fn add_rejects_overlap_with_output_dir() {
        let mut mgr = WatchManager::new();
        let err = mgr
            .add(folder("/tmp/a", false), Some(Path::new("/tmp/a/out")))
            .unwrap_err();
        assert!(matches!(err, WatchError::OverlapsOutputDir { .. }));
        let err = mgr
            .add(folder("/tmp/a", false), Some(Path::new("/tmp/a")))
            .unwrap_err();
        assert!(matches!(err, WatchError::OverlapsOutputDir { .. }));
        let err = mgr
            .add(folder("/tmp/a/sub", false), Some(Path::new("/tmp/a")))
            .unwrap_err();
        assert!(matches!(err, WatchError::OverlapsOutputDir { .. }));
    }

    #[test]
    fn add_rejects_overlap_with_existing_folder() {
        let mut mgr = WatchManager::new();
        mgr.add(folder("/tmp/a", false), None).unwrap();
        let err = mgr.add(folder("/tmp/a/sub", false), None).unwrap_err();
        assert!(matches!(err, WatchError::OverlapsExistingFolder { .. }));
        let err = mgr.add(folder("/", false), None).unwrap_err();
        assert!(matches!(err, WatchError::OverlapsExistingFolder { .. }));
    }

    #[test]
    fn validate_output_dir_rejects_overlap() {
        let mut mgr = WatchManager::new();
        mgr.add(folder("/tmp/a", false), None).unwrap();
        assert!(mgr.validate_output_dir(Path::new("/tmp/a/out")).is_err());
        assert!(mgr.validate_output_dir(Path::new("/tmp/a")).is_err());
        assert!(mgr.validate_output_dir(Path::new("/tmp/b")).is_ok());
    }

    // ─── Event classification ───────────────────────────────────────────

    #[test]
    fn classify_create_is_added() {
        use notify_debouncer_full::notify::event::CreateKind;
        let kind = EventKind::Create(CreateKind::File);
        assert_eq!(classify_event_kind(&kind), EventClass::Added);
    }

    #[test]
    fn classify_rename_to_is_added_rename_from_is_removed() {
        let to = EventKind::Modify(ModifyKind::Name(RenameMode::To));
        let from = EventKind::Modify(ModifyKind::Name(RenameMode::From));
        assert_eq!(classify_event_kind(&to), EventClass::Added);
        assert_eq!(classify_event_kind(&from), EventClass::Removed);
    }

    #[test]
    fn classify_modify_data_is_added_in_phase_4() {
        use notify_debouncer_full::notify::event::DataChange;
        let kind = EventKind::Modify(ModifyKind::Data(DataChange::Content));
        assert_eq!(classify_event_kind(&kind), EventClass::Added);
    }

    #[test]
    fn classify_remove_is_removed() {
        use notify_debouncer_full::notify::event::RemoveKind;
        let kind = EventKind::Remove(RemoveKind::File);
        assert_eq!(classify_event_kind(&kind), EventClass::Removed);
    }
}
