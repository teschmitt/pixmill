use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::formats::is_supported_input;

/// Walk a directory (optionally recursive) and collect supported image files.
pub fn collect_from_dir(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let max_depth = if recursive { usize::MAX } else { 1 };
    WalkDir::new(dir)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| is_supported_input(p))
        .collect()
}

/// Filter a flat list of paths to those we recognize as images.
pub fn filter_supported(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().filter(|p| is_supported_input(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_keeps_images_and_drops_others() {
        let input = vec![
            PathBuf::from("/tmp/a.jpg"),
            PathBuf::from("/tmp/b.txt"),
            PathBuf::from("/tmp/c.PNG"),
            PathBuf::from("/tmp/d.heic"),
            PathBuf::from("/tmp/e"),
        ];
        let out = filter_supported(input);
        assert_eq!(
            out,
            vec![
                PathBuf::from("/tmp/a.jpg"),
                PathBuf::from("/tmp/c.PNG"),
                PathBuf::from("/tmp/d.heic"),
            ]
        );
    }
}
