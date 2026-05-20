use image::DynamicImage;

/// Apply an EXIF Orientation value (1..=8) so the image's physical pixels
/// match what the metadata says the user intended to see.
pub fn apply(image: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        1 => image,
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.fliph().rotate270(),
        6 => image.rotate90(),
        7 => image.fliph().rotate90(),
        8 => image.rotate270(),
        _ => image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, RgbImage};

    #[test]
    fn orientation_6_swaps_dimensions() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(100, 50));
        let out = apply(img, 6);
        assert_eq!(out.dimensions(), (50, 100));
    }

    #[test]
    fn orientation_1_is_identity() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(100, 50));
        let out = apply(img, 1);
        assert_eq!(out.dimensions(), (100, 50));
    }

    #[test]
    fn unknown_orientation_passes_through() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(100, 50));
        let out = apply(img, 99);
        assert_eq!(out.dimensions(), (100, 50));
    }
}
