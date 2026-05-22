# Changelog

All notable changes to Pixmill are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/).

New entries are prepended by the release workflow when a PR carrying a
`release:major|minor|patch` label merges to `main`. See `README.md` →
**Release process** for details.

## [0.1.0] - 2026-05-22

Initial alpha. The core pipeline (JPEG / PNG / WebP) is tested and working;
AVIF and HEIC require optional system libs (`dav1d`, `libheif`).

### Features

- Drag-and-drop, file picker, recursive folder picker
- Live thumbnail grid for up to ~100 images
- Side-by-side preview with synced zoom and pan
- Resize (long-edge px or %), crop (aspect ratio or pixels), rotate / flip
- Target-file-size compression via binary search on encoder quality
- Watch folders with optional auto-process
- EXIF orientation baked into output pixels
- Parallel batch processing in Rust with per-file progress over Tauri channels
- Sticky settings — last-used configuration restores on launch
- Web build (SvelteKit + wasm) deployed to GitHub Pages
