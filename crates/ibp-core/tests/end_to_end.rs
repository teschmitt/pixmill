use std::path::PathBuf;

use image::{DynamicImage, ImageFormat, RgbImage};

use ibp_core::{ingest, metadata, ops, pipeline, settings, thumbnail, Settings};

fn tempdir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ibp-it-{}-{}-{}",
        std::process::id(),
        label,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_red_png(dir: &PathBuf, name: &str, w: u32, h: u32) -> PathBuf {
    let mut img = RgbImage::new(w, h);
    for p in img.pixels_mut() {
        *p = image::Rgb([220, 50, 50]);
    }
    let path = dir.join(name);
    DynamicImage::ImageRgb8(img)
        .save_with_format(&path, ImageFormat::Png)
        .unwrap();
    path
}

#[test]
fn reads_metadata_for_real_png() {
    let dir = tempdir("meta");
    let path = write_red_png(&dir, "red.png", 100, 60);
    let meta = metadata::read(&path);
    assert_eq!(meta.format, Some(ibp_core::ImageFormat::Png));
    assert_eq!(meta.width, Some(100));
    assert_eq!(meta.height, Some(60));
    assert!(meta.size_bytes.unwrap() > 0);
    assert!(meta.error.is_none());
}

#[test]
fn ingest_walks_directory_recursively() {
    let dir = tempdir("ingest");
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    write_red_png(&dir, "a.png", 10, 10);
    write_red_png(&sub, "b.png", 10, 10);
    std::fs::write(dir.join("notes.txt"), "hi").unwrap();

    let mut found = ingest::collect_from_dir(&dir, true);
    found.sort();
    assert_eq!(found.len(), 2);
    assert!(found.iter().any(|p| p.ends_with("a.png")));
    assert!(found.iter().any(|p| p.ends_with("sub/b.png")));
}

#[test]
fn ingest_non_recursive_skips_subdir() {
    let dir = tempdir("ingest-flat");
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    write_red_png(&dir, "a.png", 10, 10);
    write_red_png(&sub, "b.png", 10, 10);

    let found = ingest::collect_from_dir(&dir, false);
    assert_eq!(found.len(), 1);
    assert!(found[0].ends_with("a.png"));
}

#[test]
fn thumbnail_is_smaller_than_source() {
    let dir = tempdir("thumb");
    let path = write_red_png(&dir, "big.png", 1024, 768);
    let bytes = thumbnail::make_thumbnail_jpeg(&path, 128).unwrap();
    // JPEG SOI marker
    assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    // sanity: a 128px JPEG of solid color is small
    assert!(bytes.len() < 16 * 1024);
}

#[test]
fn thumbnail_data_url_has_jpeg_prefix() {
    let dir = tempdir("thumb-url");
    let path = write_red_png(&dir, "big.png", 512, 512);
    let url = thumbnail::make_thumbnail_data_url(&path, 64).unwrap();
    assert!(url.starts_with("data:image/jpeg;base64,"));
    assert!(url.len() > "data:image/jpeg;base64,".len() + 100);
}

#[test]
fn resize_long_edge_changes_dimensions() {
    use settings::ResizeMode;
    let img = DynamicImage::ImageRgb8(RgbImage::new(800, 400));
    let out = ops::resize::apply(img, ResizeMode::MaxLongEdge { pixels: 200 }).unwrap();
    let (w, h) = (out.width(), out.height());
    assert_eq!((w, h), (200, 100));
}

#[test]
fn batch_resizes_and_writes_to_output_dir() {
    use settings::{OutputFormat, ResizeMode};

    let src_dir = tempdir("batch-src");
    let out_dir = tempdir("batch-out");
    let paths = vec![
        write_red_png(&src_dir, "a.png", 800, 400),
        write_red_png(&src_dir, "b.png", 400, 800),
        write_red_png(&src_dir, "c.png", 200, 200),
    ];

    let mut settings = Settings::default();
    settings.resize = ResizeMode::MaxLongEdge { pixels: 100 };
    settings.output_format = OutputFormat::Jpeg;
    settings.jpeg_quality = Some(80);

    let progress = std::sync::Mutex::new(Vec::new());
    let results = pipeline::run_batch(&paths, &out_dir, &settings, |p| {
        progress.lock().unwrap().push(p);
    });

    assert_eq!(results.len(), 3);
    for r in &results {
        assert!(r.error.is_none(), "unexpected error: {:?}", r.error);
        let dest = r.destination.as_ref().unwrap();
        assert!(dest.exists());
        assert_eq!(dest.extension().and_then(|s| s.to_str()), Some("jpg"));
        let (w, h) = image::image_dimensions(dest).unwrap();
        assert!(w.max(h) == 100, "expected long-edge 100, got {w}x{h}");
    }

    let progress = progress.into_inner().unwrap();
    assert_eq!(progress.len(), 3);
    assert!(progress.iter().any(|p| p.completed == 3 && p.total == 3));
}

#[test]
fn batch_keeps_source_format_when_set_to_keep() {
    let src_dir = tempdir("batch-keep-src");
    let out_dir = tempdir("batch-keep-out");
    let paths = vec![write_red_png(&src_dir, "a.png", 100, 100)];

    let settings = Settings::default();
    let results = pipeline::run_batch(&paths, &out_dir, &settings, |_| {});
    assert_eq!(results.len(), 1);
    let dest = results[0].destination.as_ref().unwrap();
    assert_eq!(dest.extension().and_then(|s| s.to_str()), Some("png"));
}
