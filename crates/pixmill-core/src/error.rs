use std::path::PathBuf;
use thiserror::Error;

pub type IbpResult<T> = Result<T, IbpError>;

#[derive(Debug, Error)]
pub enum IbpError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("unsupported format for {path}")]
    UnsupportedFormat { path: PathBuf },

    #[error("decode failed for {path}: {source}")]
    Decode {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },

    #[error("encode failed for {path}: {source}")]
    Encode {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },

    #[error("resize failed: {0}")]
    Resize(String),

    #[error("invalid settings: {0}")]
    InvalidSettings(String),
}
