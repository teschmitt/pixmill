use std::path::{Path, PathBuf};

use ibp_core::{
    ingest, metadata,
    pipeline::{self, BatchItemResult, ProgressUpdate},
    thumbnail, ImageMetadata, Settings,
};
use tauri::{ipc::Channel, AppHandle};

use crate::persistence::{self, PersistedState};

/// Expand a list of dropped/picked paths: directories are walked (recursively if requested)
/// and the union is filtered down to supported image extensions. Then dimensions and sizes
/// are read in parallel.
#[tauri::command]
pub fn ingest_paths(paths: Vec<String>, recursive: bool) -> Vec<ImageMetadata> {
    let mut all_files: Vec<PathBuf> = Vec::new();
    for raw in paths {
        let p = PathBuf::from(raw);
        if p.is_dir() {
            all_files.extend(ingest::collect_from_dir(&p, recursive));
        } else if p.is_file() {
            all_files.push(p);
        }
    }
    let filtered: Vec<PathBuf> = ingest::filter_supported(all_files);
    dedupe_keep_order(filtered)
        .iter()
        .map(|p| metadata::read(p))
        .collect()
}

/// Re-read metadata for a single path (useful after a watch-folder event in v2).
#[tauri::command]
pub fn read_metadata(path: String) -> ImageMetadata {
    metadata::read(Path::new(&path))
}

/// Produce a base64 `data:` URL thumbnail for the given path.
#[tauri::command]
pub async fn make_thumbnail(path: String, long_edge: Option<u32>) -> Result<String, String> {
    let edge = long_edge.unwrap_or(thumbnail::THUMB_LONG_EDGE_DEFAULT);
    let path_owned = std::path::PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || {
        thumbnail::make_thumbnail_data_url(&path_owned, edge).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Load persisted settings, or null if none have been saved yet.
#[tauri::command]
pub fn load_settings(app: AppHandle) -> Result<Option<PersistedState>, String> {
    persistence::load(&app)
}

/// Save settings to disk so they're restored on next launch.
#[tauri::command]
pub fn save_settings(app: AppHandle, state: PersistedState) -> Result<(), String> {
    persistence::save(&app, &state)
}

/// Run the batch pipeline. Progress updates are streamed via the `on_progress` channel.
#[tauri::command]
pub async fn run_batch(
    paths: Vec<String>,
    out_dir: String,
    settings: Settings,
    on_progress: Channel<ProgressUpdate>,
) -> Result<Vec<BatchItemResult>, String> {
    let sources: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let out_dir = PathBuf::from(out_dir);
    let results = tauri::async_runtime::spawn_blocking(move || {
        pipeline::run_batch(&sources, &out_dir, &settings, move |update| {
            let _ = on_progress.send(update);
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(results)
}

fn dedupe_keep_order(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        if seen.insert(p.clone()) {
            out.push(p);
        }
    }
    out
}
