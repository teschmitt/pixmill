use std::io::Cursor;
use std::path::Path;

use image::DynamicImage;

use crate::{formats::ImageFormat, IbpError, IbpResult};

/// Decode an image from in-memory bytes.
///
/// `format_hint` selects the AVIF/HEIC dispatch path; for JPEG/PNG/WebP and
/// unknown formats the `image` crate's magic-byte sniff takes over. AVIF
/// requires the `avif-decode` feature and HEIC requires the `heic` feature.
///
/// `context_path` is used only for error messages — it should carry enough
/// information to identify the source (a filesystem path on native, a
/// filename on wasm).
pub fn decode_bytes(
    bytes: &[u8],
    format_hint: Option<ImageFormat>,
    context_path: &Path,
) -> IbpResult<DynamicImage> {
    match format_hint {
        Some(ImageFormat::Heic) => decode_heic_bytes(bytes, context_path),
        Some(ImageFormat::Avif) => decode_avif_bytes(bytes, context_path),
        Some(f) if f.is_decodable() => decode_via_image_crate_bytes(bytes, context_path),
        Some(_) => Err(IbpError::UnsupportedFormat {
            path: context_path.to_path_buf(),
        }),
        None => decode_via_image_crate_bytes(bytes, context_path),
    }
}

/// Decode an image file by reading its bytes and delegating to [`decode_bytes`].
#[cfg(feature = "fs")]
pub fn decode(path: &Path) -> IbpResult<DynamicImage> {
    let bytes = std::fs::read(path).map_err(|e| IbpError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    decode_bytes(&bytes, ImageFormat::from_extension(path), path)
}

fn decode_via_image_crate_bytes(bytes: &[u8], context_path: &Path) -> IbpResult<DynamicImage> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| IbpError::Io {
            path: context_path.to_path_buf(),
            source: e,
        })?
        .decode()
        .map_err(|e| IbpError::Decode {
            path: context_path.to_path_buf(),
            source: e,
        })
}

#[cfg(feature = "avif-decode")]
fn decode_avif_bytes(bytes: &[u8], context_path: &Path) -> IbpResult<DynamicImage> {
    decode_via_image_crate_bytes(bytes, context_path)
}

#[cfg(not(feature = "avif-decode"))]
fn decode_avif_bytes(_bytes: &[u8], context_path: &Path) -> IbpResult<DynamicImage> {
    Err(IbpError::UnsupportedFormat {
        path: context_path.to_path_buf(),
    })
}

#[cfg(feature = "heic")]
fn decode_heic_bytes(bytes: &[u8], context_path: &Path) -> IbpResult<DynamicImage> {
    use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};

    let lib = LibHeif::new();
    let ctx =
        HeifContext::read_from_bytes(bytes).map_err(|e| IbpError::Resize(format!("heic: {e}")))?;
    let handle = ctx
        .primary_image_handle()
        .map_err(|e| IbpError::Resize(format!("heic handle: {e}")))?;
    let image = lib
        .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)
        .map_err(|e| IbpError::Resize(format!("heic decode: {e}")))?;
    let planes = image.planes();
    let interleaved = planes
        .interleaved
        .ok_or_else(|| IbpError::Resize("heic missing interleaved plane".into()))?;
    let width = interleaved.width as u32;
    let height = interleaved.height as u32;
    let stride = interleaved.stride;
    let src = interleaved.data;
    let mut packed = Vec::with_capacity((width * height * 3) as usize);
    for row in 0..height as usize {
        let start = row * stride;
        let end = start + (width as usize) * 3;
        packed.extend_from_slice(&src[start..end]);
    }
    image::RgbImage::from_raw(width, height, packed)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| IbpError::Resize("heic buffer size mismatch".into()))
}

#[cfg(not(feature = "heic"))]
fn decode_heic_bytes(_bytes: &[u8], context_path: &Path) -> IbpResult<DynamicImage> {
    Err(IbpError::UnsupportedFormat {
        path: context_path.to_path_buf(),
    })
}
