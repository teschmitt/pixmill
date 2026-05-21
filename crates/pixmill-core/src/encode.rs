use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder, webp::WebPEncoder},
    DynamicImage, ImageEncoder,
};

use crate::{formats::ImageFormat, IbpError, IbpResult};

/// Decide which encoder to use for the output, given the source format and the
/// user's choice. `OutputFormat::Keep` falls back to the source format, with
/// HEIC/AVIF being mapped to JPEG per MVP design.
pub fn resolve_output_format(
    source: Option<ImageFormat>,
    choice: crate::settings::OutputFormat,
) -> ImageFormat {
    use crate::settings::OutputFormat as O;
    match choice {
        O::Jpeg => ImageFormat::Jpeg,
        O::Png => ImageFormat::Png,
        O::Webp => ImageFormat::Webp,
        O::Keep => match source {
            Some(ImageFormat::Heic) | Some(ImageFormat::Avif) | None => ImageFormat::Jpeg,
            Some(other) => other,
        },
    }
}

pub fn write_to_path(
    image: &DynamicImage,
    path: &Path,
    format: ImageFormat,
    settings: &crate::Settings,
) -> IbpResult<()> {
    let file = File::create(path).map_err(|e| IbpError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let writer = BufWriter::new(file);
    encode_into(image, writer, format, settings, path)
}

/// Encode `image` to an in-memory `Vec<u8>` using the same per-format logic as
/// [`write_to_path`]. `context_path` is only used for error reporting (typically
/// the source file's path).
pub fn to_bytes(
    image: &DynamicImage,
    format: ImageFormat,
    settings: &crate::Settings,
    context_path: &Path,
) -> IbpResult<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    encode_into(image, &mut buf, format, settings, context_path)?;
    Ok(buf)
}

fn encode_into<W: Write>(
    image: &DynamicImage,
    writer: W,
    format: ImageFormat,
    settings: &crate::Settings,
    context_path: &Path,
) -> IbpResult<()> {
    match format {
        ImageFormat::Jpeg => encode_jpeg(image, writer, settings, context_path),
        ImageFormat::Png => encode_png(image, writer, context_path),
        ImageFormat::Webp => encode_webp(image, writer, settings, context_path),
        ImageFormat::Avif | ImageFormat::Heic => Err(IbpError::Encode {
            path: context_path.to_path_buf(),
            source: image::ImageError::Parameter(image::error::ParameterError::from_kind(
                image::error::ParameterErrorKind::Generic(format!(
                    "{format:?} encode is not supported in the MVP"
                )),
            )),
        }),
    }
}

/// Encode an image to JPEG/WebP bytes targeting a kilobyte budget. Binary-searches
/// encoder quality (max 6 iterations, early-stop in [target*0.9, target]). If even
/// quality 1 exceeds the target, returns the quality-1 result anyway — the budget
/// is treated as soft.
pub fn to_bytes_target_size(
    image: &DynamicImage,
    format: ImageFormat,
    kilobytes: u32,
    context_path: &Path,
) -> IbpResult<Vec<u8>> {
    let target_bytes = (kilobytes as usize).saturating_mul(1024);
    match format {
        ImageFormat::Jpeg => search_jpeg_for_target(image, target_bytes, context_path),
        ImageFormat::Webp => Ok(search_webp_for_target(image, target_bytes)),
        _ => Err(IbpError::InvalidSettings(format!(
            "target file size requires JPEG or WebP output (got {format:?})"
        ))),
    }
}

pub fn write_to_path_target_size(
    image: &DynamicImage,
    path: &Path,
    format: ImageFormat,
    kilobytes: u32,
) -> IbpResult<()> {
    let bytes = to_bytes_target_size(image, format, kilobytes, path)?;
    let file = File::create(path).map_err(|e| IbpError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let mut writer = BufWriter::new(file);
    writer.write_all(&bytes).map_err(|e| IbpError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

const TARGET_SIZE_MAX_ITERS: u32 = 6;

fn search_jpeg_for_target(
    image: &DynamicImage,
    target: usize,
    context_path: &Path,
) -> IbpResult<Vec<u8>> {
    let rgb = image.to_rgb8();
    let encode = |q: u8| -> IbpResult<Vec<u8>> {
        let mut buf = Vec::with_capacity(64 * 1024);
        let encoder = JpegEncoder::new_with_quality(&mut buf, q);
        rgb.write_with_encoder(encoder)
            .map_err(|e| IbpError::Encode {
                path: context_path.to_path_buf(),
                source: e,
            })?;
        Ok(buf)
    };

    let mut lo: u8 = 1;
    let mut hi: u8 = 100;
    let tolerance_low = (target as f64 * 0.9) as usize;
    let mut best_under: Option<Vec<u8>> = None;
    let mut q1_bytes: Option<Vec<u8>> = None;

    for _ in 0..TARGET_SIZE_MAX_ITERS {
        if lo > hi {
            break;
        }
        let q = lo + (hi - lo) / 2;
        let bytes = encode(q)?;
        let size = bytes.len();
        if q == 1 {
            q1_bytes = Some(bytes.clone());
        }
        if size <= target {
            let in_band = size >= tolerance_low;
            best_under = Some(bytes);
            if in_band || q == 100 {
                break;
            }
            lo = q + 1;
        } else {
            if q == 1 {
                break;
            }
            hi = q - 1;
        }
    }

    if let Some(b) = best_under {
        return Ok(b);
    }
    if let Some(b) = q1_bytes {
        return Ok(b);
    }
    encode(1)
}

fn search_webp_for_target(image: &DynamicImage, target: usize) -> Vec<u8> {
    let has_alpha = image.color().has_alpha();
    let rgba = if has_alpha {
        Some(image.to_rgba8())
    } else {
        None
    };
    let rgb = if has_alpha {
        None
    } else {
        Some(image.to_rgb8())
    };
    let encode = |q: u8| -> Vec<u8> {
        let quality = q as f32;
        let encoded = if let Some(ref rgba) = rgba {
            webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height()).encode(quality)
        } else {
            let rgb = rgb.as_ref().expect("rgb buffer present when no alpha");
            webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height()).encode(quality)
        };
        encoded.to_vec()
    };

    let mut lo: u8 = 1;
    let mut hi: u8 = 100;
    let tolerance_low = (target as f64 * 0.9) as usize;
    let mut best_under: Option<Vec<u8>> = None;
    let mut q1_bytes: Option<Vec<u8>> = None;

    for _ in 0..TARGET_SIZE_MAX_ITERS {
        if lo > hi {
            break;
        }
        let q = lo + (hi - lo) / 2;
        let bytes = encode(q);
        let size = bytes.len();
        if q == 1 {
            q1_bytes = Some(bytes.clone());
        }
        if size <= target {
            let in_band = size >= tolerance_low;
            best_under = Some(bytes);
            if in_band || q == 100 {
                break;
            }
            lo = q + 1;
        } else {
            if q == 1 {
                break;
            }
            hi = q - 1;
        }
    }

    best_under.or(q1_bytes).unwrap_or_else(|| encode(1))
}

fn encode_jpeg<W: Write>(
    image: &DynamicImage,
    writer: W,
    settings: &crate::Settings,
    context_path: &Path,
) -> IbpResult<()> {
    let quality = settings.jpeg_quality.unwrap_or(85);
    let rgb = image.to_rgb8();
    let encoder = JpegEncoder::new_with_quality(writer, quality);
    rgb.write_with_encoder(encoder)
        .map_err(|e| IbpError::Encode {
            path: context_path.to_path_buf(),
            source: e,
        })
}

fn encode_png<W: Write>(image: &DynamicImage, writer: W, context_path: &Path) -> IbpResult<()> {
    let encoder = PngEncoder::new(writer);
    if image.color().has_alpha() {
        let rgba = image.to_rgba8();
        encoder
            .write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| IbpError::Encode {
                path: context_path.to_path_buf(),
                source: e,
            })
    } else {
        let rgb = image.to_rgb8();
        encoder
            .write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| IbpError::Encode {
                path: context_path.to_path_buf(),
                source: e,
            })
    }
}

fn encode_webp<W: Write>(
    image: &DynamicImage,
    mut writer: W,
    settings: &crate::Settings,
    context_path: &Path,
) -> IbpResult<()> {
    match settings.webp_quality {
        Some(q) => {
            let quality = q as f32;
            let encoded = if image.color().has_alpha() {
                let rgba = image.to_rgba8();
                webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height()).encode(quality)
            } else {
                let rgb = image.to_rgb8();
                webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height()).encode(quality)
            };
            writer.write_all(&encoded).map_err(|e| IbpError::Io {
                path: context_path.to_path_buf(),
                source: e,
            })
        }
        None => {
            let encoder = WebPEncoder::new_lossless(writer);
            if image.color().has_alpha() {
                let rgba = image.to_rgba8();
                encoder
                    .write_image(
                        rgba.as_raw(),
                        rgba.width(),
                        rgba.height(),
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| IbpError::Encode {
                        path: context_path.to_path_buf(),
                        source: e,
                    })
            } else {
                let rgb = image.to_rgb8();
                encoder
                    .write_image(
                        rgb.as_raw(),
                        rgb.width(),
                        rgb.height(),
                        image::ExtendedColorType::Rgb8,
                    )
                    .map_err(|e| IbpError::Encode {
                        path: context_path.to_path_buf(),
                        source: e,
                    })
            }
        }
    }
}
