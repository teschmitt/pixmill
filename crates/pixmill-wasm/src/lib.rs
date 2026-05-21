//! `wasm-bindgen` exports that expose `pixmill_core`'s byte API to JS.
//!
//! Each export mirrors a Tauri command on the desktop side so the web
//! `Platform` impl in `src/lib/platform/web/` can be a thin shim. Bytes flow
//! as `Uint8Array`, settings as plain JS objects, errors as JS `Error`s with a
//! string message — `IbpError` variants are not preserved across the boundary.

use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use js_sys::{Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;

use pixmill_core::{metadata, pipeline, settings::Settings, thumbnail, IbpError, ImageFormat};

mod error;
use error::to_js_error;

#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_panic")]
    console_error_panic_hook::set_once();
}

/// Header-only ingest. Returns a `RawMetadata`-shaped object: `{path, filename,
/// format, width, height, sizeBytes, error}`. On the web build `path` carries
/// the filename since there is no real filesystem path.
#[wasm_bindgen(js_name = ingestMetadata)]
pub fn ingest_metadata(bytes: &[u8], filename: String) -> Result<JsValue, JsError> {
    let meta = metadata::from_bytes(bytes, &filename);
    serde_wasm_bindgen::to_value(&meta).map_err(|e| JsError::new(&e.to_string()))
}

/// Decode + downscale to a JPEG thumbnail. Returns raw JPEG bytes — the JS
/// caller wraps them in a `data:` URL (or hands them to `URL.createObjectURL`).
#[wasm_bindgen(js_name = makeThumbnail)]
pub fn make_thumbnail(bytes: &[u8], filename: String, long_edge: u32) -> Result<Vec<u8>, JsError> {
    thumbnail::make_thumbnail_jpeg_bytes(bytes, Path::new(&filename), long_edge)
        .map_err(to_js_error)
}

/// Header-decode + re-wrap source bytes as a `data:` URL the webview can render
/// directly. Returns `{dataUrl, width, height}`. Mirrors the Tauri `load_source`
/// command for JPEG/PNG/WebP; AVIF/HEIC error with `UnsupportedFormat` here
/// because the `avif-decode` / `heic` features are off in the wasm build.
#[wasm_bindgen(js_name = loadSource)]
pub fn load_source(bytes: &[u8], filename: String) -> Result<JsValue, JsError> {
    let path = Path::new(&filename);
    let format = ImageFormat::from_extension(path);

    match format {
        Some(f @ (ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::Webp)) => {
            let meta = metadata::from_bytes(bytes, &filename);
            if let Some(err) = meta.error {
                return Err(JsError::new(&err));
            }
            let (Some(width), Some(height)) = (meta.width, meta.height) else {
                return Err(JsError::new("source has no readable dimensions"));
            };
            let data_url = encode_data_url(f, bytes);
            Ok(source_load_to_js(&data_url, width, height))
        }
        _ => Err(to_js_error(IbpError::UnsupportedFormat {
            path: path.to_path_buf(),
        })),
    }
}

/// Run the full pipeline (decode → ops → encode) on `bytes` and return the
/// encoded output as `{bytes, format, width, height, sizeBytes}`. JS wraps
/// the bytes in a `data:` URL for preview, or accumulates them into a ZIP
/// for batch — keeping the raw `Uint8Array` avoids the base64 round-trip
/// in the batch path.
#[wasm_bindgen(js_name = processImage)]
pub fn process_image(
    bytes: &[u8],
    filename: String,
    settings: JsValue,
) -> Result<JsValue, JsError> {
    let settings: Settings = serde_wasm_bindgen::from_value(settings)
        .map_err(|e| JsError::new(&format!("invalid settings: {e}")))?;
    let preview = pipeline::process_bytes(bytes, &filename, &settings).map_err(to_js_error)?;
    Ok(processed_image_to_js(preview))
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

fn format_to_str(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Png => "png",
        ImageFormat::Webp => "webp",
        ImageFormat::Avif => "avif",
        ImageFormat::Heic => "heic",
    }
}

// `Reflect::set` on a freshly-allocated `Object` can only fail in pathological
// host-engine states (frozen prototype, etc.), so we unwrap. A failure here
// would be a wasm-bindgen / js-sys bug, not a user-visible error.
fn source_load_to_js(data_url: &str, width: u32, height: u32) -> JsValue {
    let obj = Object::new();
    Reflect::set(&obj, &"dataUrl".into(), &JsValue::from_str(data_url)).unwrap();
    Reflect::set(&obj, &"width".into(), &JsValue::from(width)).unwrap();
    Reflect::set(&obj, &"height".into(), &JsValue::from(height)).unwrap();
    obj.into()
}

fn processed_image_to_js(preview: pipeline::PreviewBytes) -> JsValue {
    let obj = Object::new();
    // Constructing a fresh Uint8Array copies the bytes from the wasm linear
    // memory into JS memory, so the caller owns them independently of any
    // further wasm calls.
    let bytes_js = Uint8Array::from(&preview.bytes[..]);
    let size_bytes = preview.bytes.len() as u32;
    Reflect::set(&obj, &"bytes".into(), &bytes_js).unwrap();
    Reflect::set(
        &obj,
        &"format".into(),
        &JsValue::from_str(format_to_str(preview.format)),
    )
    .unwrap();
    Reflect::set(&obj, &"width".into(), &JsValue::from(preview.width)).unwrap();
    Reflect::set(&obj, &"height".into(), &JsValue::from(preview.height)).unwrap();
    Reflect::set(&obj, &"sizeBytes".into(), &JsValue::from(size_bytes)).unwrap();
    obj.into()
}
