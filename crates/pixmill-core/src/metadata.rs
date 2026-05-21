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

/// Build metadata from in-memory bytes plus the source filename. The bytes
/// are decoded just far enough to read header dimensions; nothing is written.
pub fn from_bytes(bytes: &[u8], filename: &str) -> ImageMetadata {
    let path = Path::new(filename);
    let format = ImageFormat::from_extension(path);
    let size_bytes = Some(bytes.len() as u64);

    let (width, height, error) = if format.is_some_and(|f| f.is_decodable()) {
        match image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format() {
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
        filename: filename.to_string(),
        format,
        width,
        height,
        size_bytes,
        error,
    }
}

/// Read metadata about an image on disk. Streams just the header for dimension
/// reading instead of loading the whole file, so it scales to large batches.
#[cfg(feature = "fs")]
pub fn read(path: &Path) -> ImageMetadata {
    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let format = ImageFormat::from_extension(path);
    let size_bytes = std::fs::metadata(path).ok().map(|m| m.len());

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

#[cfg(feature = "fs")]
pub fn read_many(paths: &[PathBuf]) -> Vec<ImageMetadata> {
    use rayon::prelude::*;
    paths.par_iter().map(|p| read(p)).collect()
}
