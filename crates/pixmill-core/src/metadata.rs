use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::formats::ImageFormat;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadata {
    pub path: PathBuf,
    pub filename: String,
    pub format: Option<ImageFormat>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size_bytes: Option<u64>,
    pub error: Option<String>,
}

pub fn read(path: &Path) -> ImageMetadata {
    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let format = ImageFormat::from_extension(path);
    let size_bytes = std::fs::metadata(path).ok().map(|m| m.len());

    // Only decode headers for formats the image crate supports in this MVP.
    let (width, height, error) = if format.is_some_and(|f| f.is_decodable()) {
        match image::ImageReader::open(path).and_then(|r| r.with_guessed_format()) {
            Ok(reader) => match reader.into_dimensions() {
                Ok((w, h)) => (Some(w), Some(h), None),
                Err(e) => (None, None, Some(format!("{e}"))),
            },
            Err(e) => (None, None, Some(format!("{e}"))),
        }
    } else {
        (None, None, None)
    };

    ImageMetadata {
        path: path.to_path_buf(),
        filename,
        format,
        width,
        height,
        size_bytes,
        error,
    }
}

pub fn read_many(paths: &[PathBuf]) -> Vec<ImageMetadata> {
    use rayon::prelude::*;
    paths.par_iter().map(|p| read(p)).collect()
}
