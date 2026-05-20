use image::{DynamicImage, GenericImageView};

use crate::{settings::CropMode, IbpResult};

pub fn apply(image: DynamicImage, mode: CropMode) -> IbpResult<DynamicImage> {
    match mode {
        CropMode::None => Ok(image),
        CropMode::Pixels { width, height } => Ok(center_crop(&image, width, height)),
        CropMode::AspectRatio { width, height } => {
            let (target_w, target_h) = aspect_crop_dims(&image, width, height);
            Ok(center_crop(&image, target_w, target_h))
        }
    }
}

fn aspect_crop_dims(image: &DynamicImage, ratio_w: u32, ratio_h: u32) -> (u32, u32) {
    let (img_w, img_h) = image.dimensions();
    if ratio_w == 0 || ratio_h == 0 {
        return (img_w, img_h);
    }
    let ratio = ratio_w as f32 / ratio_h as f32;
    let img_ratio = img_w as f32 / img_h as f32;
    if img_ratio > ratio {
        let new_w = (img_h as f32 * ratio).round() as u32;
        (new_w.min(img_w), img_h)
    } else {
        let new_h = (img_w as f32 / ratio).round() as u32;
        (img_w, new_h.min(img_h))
    }
}

fn center_crop(image: &DynamicImage, target_w: u32, target_h: u32) -> DynamicImage {
    let (img_w, img_h) = image.dimensions();
    let crop_w = target_w.min(img_w);
    let crop_h = target_h.min(img_h);
    let x = (img_w.saturating_sub(crop_w)) / 2;
    let y = (img_h.saturating_sub(crop_h)) / 2;
    image.crop_imm(x, y, crop_w, crop_h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbImage};

    fn solid_image(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::new(w, h))
    }

    #[test]
    fn aspect_ratio_keeps_smaller_side_full() {
        let img = solid_image(400, 200);
        let (w, h) = aspect_crop_dims(&img, 1, 1);
        assert_eq!((w, h), (200, 200));
    }

    #[test]
    fn pixel_crop_centered() {
        let img = solid_image(100, 100);
        let cropped = apply(img, CropMode::Pixels { width: 40, height: 40 }).unwrap();
        assert_eq!(cropped.dimensions(), (40, 40));
    }
}
