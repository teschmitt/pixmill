use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{codecs::jpeg::JpegEncoder, DynamicImage};

use crate::{IbpError, IbpResult};

pub const THUMB_LONG_EDGE_DEFAULT: u32 = 256;

/// Decode and produce a JPEG-encoded thumbnail of at most `long_edge` px on
/// the long side from in-memory bytes. `context_path` is for error reporting.
pub fn make_thumbnail_jpeg_bytes(
    bytes: &[u8],
    context_path: &Path,
    long_edge: u32,
) -> IbpResult<Vec<u8>> {
    let img = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| IbpError::Io {
            path: context_path.to_path_buf(),
            source: e,
        })?
        .decode()
        .map_err(|e| IbpError::Decode {
            path: context_path.to_path_buf(),
            source: e,
        })?;
    let thumb = crate::ops::resize::apply(
        img,
        crate::settings::ResizeMode::MaxLongEdge { pixels: long_edge },
    )?;
    encode_jpeg(&thumb, 80, context_path)
}

/// Wrap JPEG bytes in a `data:image/jpeg;base64,...` URL.
pub fn jpeg_bytes_to_data_url(bytes: &[u8]) -> String {
    let mut url = String::with_capacity(bytes.len() * 4 / 3 + 32);
    url.push_str("data:image/jpeg;base64,");
    STANDARD.encode_string(bytes, &mut url);
    url
}

#[cfg(feature = "fs")]
pub fn make_thumbnail_jpeg(path: &Path, long_edge: u32) -> IbpResult<Vec<u8>> {
    let bytes = std::fs::read(path).map_err(|e| IbpError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    make_thumbnail_jpeg_bytes(&bytes, path, long_edge)
}

#[cfg(feature = "fs")]
pub fn make_thumbnail_data_url(path: &Path, long_edge: u32) -> IbpResult<String> {
    let bytes = make_thumbnail_jpeg(path, long_edge)?;
    Ok(jpeg_bytes_to_data_url(&bytes))
}

fn encode_jpeg(image: &DynamicImage, quality: u8, path: &Path) -> IbpResult<Vec<u8>> {
    let mut buf = Vec::with_capacity(64 * 1024);
    let rgb = image.to_rgb8();
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    rgb.write_with_encoder(encoder)
        .map_err(|e| IbpError::Encode {
            path: path.to_path_buf(),
            source: e,
        })?;
    Ok(buf)
}
