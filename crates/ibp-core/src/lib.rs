pub mod decode;
pub mod encode;
pub mod error;
pub mod exif;
pub mod formats;
pub mod ingest;
pub mod metadata;
pub mod ops;
pub mod pipeline;
pub mod settings;
pub mod thumbnail;

pub use error::{IbpError, IbpResult};
pub use formats::ImageFormat;
pub use metadata::{read as read_metadata, read_many as read_metadata_many, ImageMetadata};
pub use settings::{CropMode, OutputFormat, ResizeMode, RotateMode, Settings};
