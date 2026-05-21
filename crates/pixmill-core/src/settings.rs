use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A directory the user has asked the app to keep an eye on for incoming
/// images. Persisted across launches; live-watching itself is wired up by
/// `src-tauri/src/watch.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedFolder {
    pub path: PathBuf,
    pub recursive: bool,
    pub auto_process: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ResizeMode {
    #[default]
    None,
    MaxLongEdge {
        pixels: u32,
    },
    Percentage {
        percent: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum CompressionMode {
    #[default]
    Manual,
    TargetFileSize {
        kilobytes: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum CropMode {
    #[default]
    None,
    AspectRatio {
        width: u32,
        height: u32,
    },
    Pixels {
        width: u32,
        height: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RotateMode {
    #[default]
    None,
    Cw90,
    Cw180,
    Cw270,
    FlipH,
    FlipV,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Keep,
    Jpeg,
    Png,
    Webp,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub resize: ResizeMode,
    pub crop: CropMode,
    pub rotate: RotateMode,
    pub output_format: OutputFormat,
    #[serde(default)]
    pub compression: CompressionMode,
    pub jpeg_quality: Option<u8>,
    pub webp_quality: Option<u8>,
    pub preserve_exif: bool,
}

impl Settings {
    pub fn validate(&self) -> crate::IbpResult<()> {
        if let ResizeMode::MaxLongEdge { pixels } = self.resize {
            if pixels == 0 {
                return Err(crate::IbpError::InvalidSettings(
                    "resize max long-edge must be > 0".into(),
                ));
            }
        }
        if let ResizeMode::Percentage { percent } = self.resize {
            if percent == 0 || percent > 1000 {
                return Err(crate::IbpError::InvalidSettings(
                    "resize percentage must be between 1 and 1000".into(),
                ));
            }
        }
        if let CompressionMode::TargetFileSize { kilobytes } = self.compression {
            if kilobytes == 0 {
                return Err(crate::IbpError::InvalidSettings(
                    "target file size must be > 0 KB".into(),
                ));
            }
            if matches!(self.output_format, OutputFormat::Png) {
                return Err(crate::IbpError::InvalidSettings(
                    "target file size requires JPEG or WebP output".into(),
                ));
            }
        }
        if let Some(q) = self.jpeg_quality {
            if q == 0 || q > 100 {
                return Err(crate::IbpError::InvalidSettings(
                    "jpeg quality must be 1..=100".into(),
                ));
            }
        }
        if let Some(q) = self.webp_quality {
            if q == 0 || q > 100 {
                return Err(crate::IbpError::InvalidSettings(
                    "webp quality must be 1..=100".into(),
                ));
            }
        }
        Ok(())
    }
}
