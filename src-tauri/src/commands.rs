use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use pixmill_core::{
    ingest, metadata,
    pipeline::{self, BatchItemResult, ProgressUpdate},
    thumbnail, ImageFormat, ImageMetadata, Settings,
};
use serde::{Deserialize, Serialize};
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

/// Result of an in-memory preview run: the encoded output as a base64 data URL plus
/// the dimensions, size, and format the user would get if they ran the batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: usize,
    pub format: ImageFormat,
}

/// Full-resolution source loaded for the preview modal. Webview-native formats
/// (JPEG/PNG/WebP) are returned as their raw bytes in a data URL so the webview's
/// own decoder handles orientation. Other formats (AVIF/HEIC) are decoded via
/// pixmill-core, oriented, and re-encoded as PNG so the webview can display them at all.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLoad {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

#[tauri::command]
pub async fn load_source(path: String) -> Result<SourceLoad, String> {
    let source = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || load_source_blocking(&source))
        .await
        .map_err(|e| e.to_string())?
}

fn load_source_blocking(source: &Path) -> Result<SourceLoad, String> {
    let format = pixmill_core::ImageFormat::from_extension(source);
    match format {
        // Webview-native: pass raw bytes through. The webview decoder applies EXIF
        // orientation, so naturalWidth/Height on the <img> will be post-orientation.
        // We can't cheaply know post-orientation dims here, so we report the raw
        // dimensions — JS reads naturalWidth on img.onload for the canonical value.
        Some(ImageFormat::Jpeg) | Some(ImageFormat::Png) | Some(ImageFormat::Webp) => {
            let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
            let (width, height) = image::image_dimensions(source).map_err(|e| e.to_string())?;
            Ok(SourceLoad {
                data_url: encode_data_url(format.unwrap(), &bytes),
                width,
                height,
            })
        }
        // AVIF/HEIC (with the respective Cargo features) or anything else: decode,
        // bake in EXIF orientation, re-encode as PNG so the webview can display it.
        _ => {
            let img = pixmill_core::decode::decode(source).map_err(|e| e.to_string())?;
            let img = match pixmill_core::exif::read_orientation(source) {
                Some(o) => pixmill_core::ops::orient::apply(img, o),
                None => img,
            };
            let bytes = pixmill_core::encode::to_bytes(
                &img,
                ImageFormat::Png,
                &Settings::default(),
                source,
            )
            .map_err(|e| e.to_string())?;
            Ok(SourceLoad {
                data_url: encode_data_url(ImageFormat::Png, &bytes),
                width: img.width(),
                height: img.height(),
            })
        }
    }
}

/// Run the pipeline on a single file with the given settings and return an in-memory
/// preview (base64 data URL + metadata). No file is written.
#[tauri::command]
pub async fn preview_one(path: String, settings: Settings) -> Result<PreviewResult, String> {
    let source = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || {
        let preview =
            pipeline::process_one_to_bytes(&source, &settings).map_err(|e| e.to_string())?;
        let size_bytes = preview.bytes.len();
        Ok(PreviewResult {
            data_url: encode_data_url(preview.format, &preview.bytes),
            width: preview.width,
            height: preview.height,
            size_bytes,
            format: preview.format,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn encode_data_url(format: ImageFormat, bytes: &[u8]) -> String {
    let mime = match format {
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Png => "image/png",
        ImageFormat::Webp => "image/webp",
        ImageFormat::Avif => "image/avif",
        ImageFormat::Heic => "image/heic",
    };
    let mut url = String::with_capacity(bytes.len() * 4 / 3 + 32);
    url.push_str("data:");
    url.push_str(mime);
    url.push_str(";base64,");
    STANDARD.encode_string(bytes, &mut url);
    url
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
