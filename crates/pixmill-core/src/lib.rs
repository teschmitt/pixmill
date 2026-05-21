pub mod decode;
pub mod encode;
pub mod error;
pub mod exif;
pub mod formats;
#[cfg(feature = "fs")]
pub mod ingest;
pub mod metadata;
pub mod ops;
pub mod pipeline;
pub mod settings;
pub mod thumbnail;

pub use error::{IbpError, IbpResult};
pub use formats::ImageFormat;
pub use metadata::ImageMetadata;
#[cfg(feature = "fs")]
pub use metadata::{read as read_metadata, read_many as read_metadata_many};
pub use settings::{CompressionMode, CropMode, OutputFormat, ResizeMode, RotateMode, Settings};
