use image::DynamicImage;

use crate::settings::RotateMode;

pub fn apply(image: DynamicImage, mode: RotateMode) -> DynamicImage {
    match mode {
        RotateMode::None => image,
        RotateMode::Cw90 => image.rotate90(),
        RotateMode::Cw180 => image.rotate180(),
        RotateMode::Cw270 => image.rotate270(),
        RotateMode::FlipH => image.fliph(),
        RotateMode::FlipV => image.flipv(),
    }
}
