use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Jpeg,
    Png,
    Webp,
    Avif,
    Heic,
}

impl ImageFormat {
    pub fn from_extension(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" | "jpe" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "webp" => Some(Self::Webp),
            "avif" => Some(Self::Avif),
            "heic" | "heif" => Some(Self::Heic),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Avif => "avif",
            Self::Heic => "heic",
        }
    }

    /// Whether this MVP build can decode this format. AVIF/HEIC are wired in Phase 7.
    pub fn is_decodable(self) -> bool {
        matches!(self, Self::Jpeg | Self::Png | Self::Webp)
    }

    /// Whether this MVP build can encode this format. HEIC encode is intentionally
    /// unsupported; AVIF/HEIC inputs are converted to JPEG on output per design.
    pub fn is_encodable(self) -> bool {
        matches!(self, Self::Jpeg | Self::Png | Self::Webp)
    }
}

pub const SUPPORTED_INPUT_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "jpe", "png", "webp", "avif", "heic", "heif"];

pub fn is_supported_input(path: &Path) -> bool {
    ImageFormat::from_extension(path).is_some()
}
