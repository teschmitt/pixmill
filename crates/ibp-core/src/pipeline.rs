use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{decode, encode, formats::ImageFormat, ops, IbpResult, Settings};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItemResult {
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdate {
    pub completed: usize,
    pub total: usize,
    pub item: BatchItemResult,
}

/// Plan the output path for a source file. On collision, append `_1`, `_2`, ...
pub fn plan_output_path(source: &Path, out_dir: &Path, ext: &str) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_string());
    let mut candidate = out_dir.join(format!("{stem}.{ext}"));
    let mut n = 1u32;
    while candidate.exists() {
        candidate = out_dir.join(format!("{stem}_{n}.{ext}"));
        n += 1;
        if n > 9999 {
            break;
        }
    }
    candidate
}

/// Process a single file end-to-end.
pub fn process_one(source: &Path, out_dir: &Path, settings: &Settings) -> IbpResult<PathBuf> {
    settings.validate()?;
    let exif_orientation = if settings.preserve_exif {
        crate::exif::read_orientation(source)
    } else {
        None
    };
    let image = decode::decode(source)?;
    let image = ops::apply_all(image, settings, exif_orientation)?;
    let source_format = ImageFormat::from_extension(source);
    let out_format = encode::resolve_output_format(source_format, settings.output_format);
    let out_path = plan_output_path(source, out_dir, out_format.extension());
    encode::write_to_path(&image, &out_path, out_format, settings)?;
    Ok(out_path)
}

/// Process a list of files in parallel, calling `on_progress` after each one finishes.
///
/// `on_progress` is called from worker threads. The total is fixed at the start, and
/// `completed` is monotonically increasing.
pub fn run_batch<F>(
    sources: &[PathBuf],
    out_dir: &Path,
    settings: &Settings,
    on_progress: F,
) -> Vec<BatchItemResult>
where
    F: Fn(ProgressUpdate) + Sync + Send,
{
    // Create the output directory up front so individual workers don't race.
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        return sources
            .iter()
            .map(|s| BatchItemResult {
                source: s.clone(),
                destination: None,
                error: Some(format!("could not create output dir: {e}")),
            })
            .collect();
    }

    let total = sources.len();
    let completed = AtomicUsize::new(0);

    sources
        .par_iter()
        .map(|src| {
            let result = match process_one(src, out_dir, settings) {
                Ok(dest) => BatchItemResult {
                    source: src.clone(),
                    destination: Some(dest),
                    error: None,
                },
                Err(e) => BatchItemResult {
                    source: src.clone(),
                    destination: None,
                    error: Some(format!("{e}")),
                },
            };
            let n = completed.fetch_add(1, Ordering::SeqCst) + 1;
            on_progress(ProgressUpdate {
                completed: n,
                total,
                item: result.clone(),
            });
            result
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn plans_collision_free_filename() {
        let tmp = tempdir();
        let src = PathBuf::from("/some/where/photo.jpg");
        let first = plan_output_path(&src, &tmp, "jpg");
        fs::write(&first, b"x").unwrap();
        let second = plan_output_path(&src, &tmp, "jpg");
        assert_ne!(first, second);
        assert!(second.file_name().unwrap().to_string_lossy().contains("_1"));
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ibp-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
