use fast_image_resize::images::Image;
use fast_image_resize::{IntoImageView, PixelType, ResizeOptions, Resizer};
use image::{DynamicImage, GenericImageView};

// IntoImageView is brought into scope so we can call `pixel_type()` on `DynamicImage`.
#[allow(unused_imports)]
use IntoImageView as _;

use crate::{settings::ResizeMode, IbpError, IbpResult};

pub fn apply(image: DynamicImage, mode: ResizeMode) -> IbpResult<DynamicImage> {
    let (target_w, target_h) = match mode {
        ResizeMode::None => return Ok(image),
        ResizeMode::MaxLongEdge { pixels } => fit_long_edge(image.dimensions(), pixels),
        ResizeMode::Percentage { percent } => scale_by_percent(image.dimensions(), percent),
    };
    if (target_w, target_h) == image.dimensions() {
        return Ok(image);
    }
    resize_lanczos(&image, target_w, target_h)
}

fn fit_long_edge((w, h): (u32, u32), max_edge: u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (w, h);
    }
    let long = w.max(h);
    if long <= max_edge {
        return (w, h);
    }
    let scale = max_edge as f64 / long as f64;
    let nw = ((w as f64) * scale).round().max(1.0) as u32;
    let nh = ((h as f64) * scale).round().max(1.0) as u32;
    (nw, nh)
}

fn scale_by_percent((w, h): (u32, u32), percent: u32) -> (u32, u32) {
    let p = percent as f64 / 100.0;
    let nw = ((w as f64) * p).round().max(1.0) as u32;
    let nh = ((h as f64) * p).round().max(1.0) as u32;
    (nw, nh)
}

fn resize_lanczos(image: &DynamicImage, target_w: u32, target_h: u32) -> IbpResult<DynamicImage> {
    let pixel_type: PixelType = image
        .pixel_type()
        .ok_or_else(|| IbpError::Resize("unsupported pixel type for source".into()))?;
    let mut dst = Image::new(target_w, target_h, pixel_type);
    let mut resizer = Resizer::new();
    resizer
        .resize(image, &mut dst, &ResizeOptions::new())
        .map_err(|e| IbpError::Resize(format!("{e}")))?;
    let buffer = dst.into_vec();
    match pixel_type {
        PixelType::U8x3 => image::RgbImage::from_raw(target_w, target_h, buffer)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| IbpError::Resize("rgb buffer size mismatch".into())),
        PixelType::U8x4 => image::RgbaImage::from_raw(target_w, target_h, buffer)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| IbpError::Resize("rgba buffer size mismatch".into())),
        PixelType::U8 => image::GrayImage::from_raw(target_w, target_h, buffer)
            .map(DynamicImage::ImageLuma8)
            .ok_or_else(|| IbpError::Resize("luma buffer size mismatch".into())),
        PixelType::U8x2 => image::GrayAlphaImage::from_raw(target_w, target_h, buffer)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(|| IbpError::Resize("luma-alpha buffer size mismatch".into())),
        other => Err(IbpError::Resize(format!(
            "unsupported pixel type {other:?} from resizer"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_edge_landscape() {
        assert_eq!(fit_long_edge((4000, 2000), 1000), (1000, 500));
    }

    #[test]
    fn long_edge_portrait() {
        assert_eq!(fit_long_edge((2000, 4000), 1000), (500, 1000));
    }

    #[test]
    fn long_edge_no_upscale() {
        assert_eq!(fit_long_edge((400, 200), 1000), (400, 200));
    }

    #[test]
    fn percentage_half() {
        assert_eq!(scale_by_percent((400, 200), 50), (200, 100));
    }
}
