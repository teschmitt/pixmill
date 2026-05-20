use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{codecs::jpeg::JpegEncoder, DynamicImage};

use crate::{IbpError, IbpResult};

pub const THUMB_LONG_EDGE_DEFAULT: u32 = 256;

/// Return a base64 `data:` URL for a thumbnail of the given image.
pub fn make_thumbnail_data_url(path: &Path, long_edge: u32) -> IbpResult<String> {
    let bytes = make_thumbnail_jpeg(path, long_edge)?;
    let mut url = String::with_capacity(bytes.len() * 4 / 3 + 32);
    url.push_str("data:image/jpeg;base64,");
    STANDARD.encode_string(&bytes, &mut url);
    Ok(url)
}

/// Decode an image and return a JPEG-encoded thumbnail no larger than `long_edge` px.
pub fn make_thumbnail_jpeg(path: &Path, long_edge: u32) -> IbpResult<Vec<u8>> {
    let img = image::ImageReader::open(path)
        .map_err(|e| IbpError::Io {
            path: path.to_path_buf(),
            source: e,
        })?
        .with_guessed_format()
        .map_err(|e| IbpError::Io {
            path: path.to_path_buf(),
            source: e,
        })?
        .decode()
        .map_err(|e| IbpError::Decode {
            path: path.to_path_buf(),
            source: e,
        })?;
    let thumb = crate::ops::resize::apply(
        img,
        crate::settings::ResizeMode::MaxLongEdge { pixels: long_edge },
    )?;
    encode_jpeg(&thumb, 80, path)
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
