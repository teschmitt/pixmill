use std::fs::File;
use std::io::BufWriter;
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
    match format {
        ImageFormat::Jpeg => write_jpeg(image, writer, settings, path),
        ImageFormat::Png => write_png(image, writer, path),
        ImageFormat::Webp => write_webp(image, writer, settings, path),
        ImageFormat::Avif | ImageFormat::Heic => Err(IbpError::Encode {
            path: path.to_path_buf(),
            source: image::ImageError::Parameter(image::error::ParameterError::from_kind(
                image::error::ParameterErrorKind::Generic(format!(
                    "{format:?} encode is not supported in the MVP"
                )),
            )),
        }),
    }
}

fn write_jpeg(
    image: &DynamicImage,
    writer: BufWriter<File>,
    settings: &crate::Settings,
    path: &Path,
) -> IbpResult<()> {
    let quality = settings.jpeg_quality.unwrap_or(85);
    let rgb = image.to_rgb8();
    let encoder = JpegEncoder::new_with_quality(writer, quality);
    rgb.write_with_encoder(encoder)
        .map_err(|e| IbpError::Encode {
            path: path.to_path_buf(),
            source: e,
        })
}

fn write_png(image: &DynamicImage, writer: BufWriter<File>, path: &Path) -> IbpResult<()> {
    // Keep alpha if present; otherwise RGB8.
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
                path: path.to_path_buf(),
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
                path: path.to_path_buf(),
                source: e,
            })
    }
}

fn write_webp(
    image: &DynamicImage,
    mut writer: BufWriter<File>,
    settings: &crate::Settings,
    path: &Path,
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
            std::io::Write::write_all(&mut writer, &encoded).map_err(|e| IbpError::Io {
                path: path.to_path_buf(),
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
                        path: path.to_path_buf(),
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
                        path: path.to_path_buf(),
                        source: e,
                    })
            }
        }
    }
}
