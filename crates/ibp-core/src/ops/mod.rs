pub mod crop;
pub mod orient;
pub mod resize;
pub mod rotate;

use image::DynamicImage;

use crate::{IbpResult, Settings};

/// Apply all transformations in canonical order:
/// EXIF orientation → user rotate → crop → resize.
pub fn apply_all(
    image: DynamicImage,
    settings: &Settings,
    exif_orientation: Option<u16>,
) -> IbpResult<DynamicImage> {
    let image = if settings.preserve_exif {
        if let Some(o) = exif_orientation {
            orient::apply(image, o)
        } else {
            image
        }
    } else {
        image
    };
    let image = rotate::apply(image, settings.rotate);
    let image = crop::apply(image, settings.crop)?;
    let image = resize::apply(image, settings.resize)?;
    Ok(image)
}
