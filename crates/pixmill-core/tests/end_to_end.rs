use std::path::PathBuf;

use image::{DynamicImage, ImageFormat, RgbImage};

use pixmill_core::{ingest, metadata, ops, pipeline, settings, thumbnail, Settings};

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
    assert_eq!(meta.format, Some(pixmill_core::ImageFormat::Png));
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

// Solid-color PNGs compress to ~nothing in both lossy and lossless modes, so
// the size comparison would be noise. Pseudo-random per-pixel RGB defeats the
// lossless predictor + entropy coder while lossy DCT quantizes the noise away.
fn write_noisy_png(dir: &PathBuf, name: &str, w: u32, h: u32) -> PathBuf {
    let mut img = RgbImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let r = ((x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503)) & 0xFF) as u8;
        let g = ((x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263)) & 0xFF) as u8;
        let b = ((x.wrapping_mul(2246822519) ^ y.wrapping_mul(3266489917)) & 0xFF) as u8;
        *p = image::Rgb([r, g, b]);
    }
    let path = dir.join(name);
    DynamicImage::ImageRgb8(img)
        .save_with_format(&path, ImageFormat::Png)
        .unwrap();
    path
}

#[test]
fn webp_lossy_is_smaller_than_lossless() {
    use settings::OutputFormat;

    let src_dir = tempdir("webp-lossy-src");
    let lossy_out = tempdir("webp-lossy-out");
    let lossless_out = tempdir("webp-lossless-out");
    let src = write_noisy_png(&src_dir, "noise.png", 256, 256);

    let mut lossy = Settings::default();
    lossy.output_format = OutputFormat::Webp;
    lossy.webp_quality = Some(60);

    let mut lossless = Settings::default();
    lossless.output_format = OutputFormat::Webp;
    lossless.webp_quality = None;

    let lossy_results = pipeline::run_batch(&[src.clone()], &lossy_out, &lossy, |_| {});
    let lossless_results = pipeline::run_batch(&[src.clone()], &lossless_out, &lossless, |_| {});

    let lossy_path = lossy_results[0].destination.as_ref().unwrap();
    let lossless_path = lossless_results[0].destination.as_ref().unwrap();
    assert_eq!(
        lossy_path.extension().and_then(|s| s.to_str()),
        Some("webp")
    );
    assert_eq!(
        lossless_path.extension().and_then(|s| s.to_str()),
        Some("webp")
    );

    image::open(lossy_path).expect("lossy decode roundtrip");
    image::open(lossless_path).expect("lossless decode roundtrip");

    let lossy_size = std::fs::metadata(lossy_path).unwrap().len();
    let lossless_size = std::fs::metadata(lossless_path).unwrap().len();
    assert!(
        lossy_size < lossless_size,
        "expected lossy ({lossy_size} bytes) < lossless ({lossless_size} bytes)",
    );
}

#[test]
fn webp_lossy_preserves_alpha_channel() {
    use settings::OutputFormat;

    let src_dir = tempdir("webp-alpha-src");
    let out_dir = tempdir("webp-alpha-out");

    let mut img = image::RgbaImage::new(32, 32);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([220, 50, 50, ((x + y) * 4).min(255) as u8]);
    }
    let src = src_dir.join("alpha.png");
    DynamicImage::ImageRgba8(img)
        .save_with_format(&src, ImageFormat::Png)
        .unwrap();

    let mut settings = Settings::default();
    settings.output_format = OutputFormat::Webp;
    settings.webp_quality = Some(80);

    let results = pipeline::run_batch(&[src], &out_dir, &settings, |_| {});
    let dest = results[0].destination.as_ref().unwrap();
    let decoded = image::open(dest).unwrap();
    assert!(decoded.color().has_alpha(), "alpha lost on roundtrip");
}

#[test]
fn target_size_jpeg_hits_budget() {
    use settings::{CompressionMode, OutputFormat};

    let src_dir = tempdir("tsize-jpeg-src");
    let out_dir = tempdir("tsize-jpeg-out");
    let src = write_noisy_png(&src_dir, "noise.png", 512, 512);

    let mut s = Settings::default();
    s.compression = CompressionMode::TargetFileSize { kilobytes: 80 };
    s.output_format = OutputFormat::Jpeg;

    let results = pipeline::run_batch(&[src], &out_dir, &s, |_| {});
    assert_eq!(results.len(), 1);
    let dest = results[0]
        .destination
        .as_ref()
        .expect("destination written");
    let size = std::fs::metadata(dest).unwrap().len() as usize;
    assert!(
        size <= 80 * 1024,
        "expected JPEG output <= 80 KB, got {size} bytes"
    );
    image::open(dest).expect("output decodes");
}

#[test]
fn target_size_webp_hits_budget() {
    use settings::{CompressionMode, OutputFormat};

    let src_dir = tempdir("tsize-webp-src");
    let out_dir = tempdir("tsize-webp-out");
    let src = write_noisy_png(&src_dir, "noise.png", 512, 512);

    let mut s = Settings::default();
    s.compression = CompressionMode::TargetFileSize { kilobytes: 40 };
    s.output_format = OutputFormat::Webp;

    let results = pipeline::run_batch(&[src], &out_dir, &s, |_| {});
    let dest = results[0]
        .destination
        .as_ref()
        .expect("destination written");
    let size = std::fs::metadata(dest).unwrap().len() as usize;
    assert!(
        size <= 40 * 1024,
        "expected WebP output <= 40 KB, got {size} bytes"
    );
    image::open(dest).expect("output decodes");
}

#[test]
fn target_size_unreachable_falls_back_to_q1() {
    use settings::{CompressionMode, OutputFormat};

    let src_dir = tempdir("tsize-fallback-src");
    let out_dir = tempdir("tsize-fallback-out");
    let src = write_noisy_png(&src_dir, "noise.png", 1024, 1024);

    let mut s = Settings::default();
    s.compression = CompressionMode::TargetFileSize { kilobytes: 1 };
    s.output_format = OutputFormat::Jpeg;

    let results = pipeline::run_batch(&[src], &out_dir, &s, |_| {});
    let result = &results[0];
    assert!(
        result.error.is_none(),
        "expected fallback success, got error: {:?}",
        result.error
    );
    let dest = result.destination.as_ref().expect("destination written");
    assert!(dest.exists());
    image::open(dest).expect("q=1 output decodes");
}

#[test]
fn target_size_rejects_png_output() {
    use settings::{CompressionMode, OutputFormat};

    let mut s = Settings::default();
    s.compression = CompressionMode::TargetFileSize { kilobytes: 100 };
    s.output_format = OutputFormat::Png;

    let err = s
        .validate()
        .expect_err("PNG + target size must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("JPEG or WebP"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn process_one_to_bytes_returns_resized_jpeg() {
    use settings::{OutputFormat, ResizeMode, RotateMode};

    let src_dir = tempdir("preview-bytes-src");
    let src = write_red_png(&src_dir, "a.png", 800, 400);

    let mut s = Settings::default();
    s.resize = ResizeMode::MaxLongEdge { pixels: 64 };
    s.rotate = RotateMode::Cw90;
    s.output_format = OutputFormat::Jpeg;
    s.jpeg_quality = Some(80);

    let preview = pipeline::process_one_to_bytes(&src, &s).expect("preview produces bytes");
    assert_eq!(preview.format, pixmill_core::ImageFormat::Jpeg);
    // JPEG SOI marker.
    assert_eq!(&preview.bytes[0..2], &[0xFF, 0xD8]);

    // After cw90 on 800x400 we get 400x800; long-edge 64 then yields 32x64.
    assert_eq!((preview.width, preview.height), (32, 64));
    let decoded = image::load_from_memory(&preview.bytes).expect("re-decode preview bytes");
    assert_eq!((decoded.width(), decoded.height()), (32, 64));
}

#[test]
fn target_size_per_file_error_on_keep_png_source() {
    use settings::CompressionMode;

    let src_dir = tempdir("tsize-keep-png-src");
    let out_dir = tempdir("tsize-keep-png-out");
    let src = write_red_png(&src_dir, "a.png", 100, 100);

    let mut s = Settings::default();
    s.compression = CompressionMode::TargetFileSize { kilobytes: 50 };
    // OutputFormat::Keep + PNG source -> resolves to PNG -> per-file error.

    let results = pipeline::run_batch(&[src], &out_dir, &s, |_| {});
    let result = &results[0];
    assert!(result.destination.is_none(), "no destination expected");
    let err = result.error.as_ref().expect("per-file error expected");
    assert!(
        err.contains("JPEG or WebP"),
        "unexpected error message: {err}"
    );
}
