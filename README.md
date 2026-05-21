# Pixmill

Drop a pile of photos, set resize / crop / rotate, hit Run. Cross-platform
desktop app built with Tauri 2 and a Rust image pipeline — small binary, no
Electron, no servers.

> **Status:** alpha. Core pipeline (JPEG / PNG / WebP) is tested and working.
> AVIF + HEIC need optional system libs. See [`PLAN.md`](./PLAN.md) for what's
> shipped and what's next.

## Features

- 🗂 Drag-and-drop, file picker, recursive folder picker
- 🖼 Live thumbnail grid for up to ~100 images at a time
- ✂ Resize (long-edge px or %), crop (aspect ratio or pixels), rotate / flip
- 🧭 EXIF orientation baked into output pixels — no more sideways portraits
- ⚡ Parallel batch processing in Rust (`rayon`), per-file progress over Tauri channels
- 💾 Originals are never touched — output goes to a folder you choose
- 🔁 Sticky settings — your last-used configuration restores on launch
- 🖥 Native window: macOS · Windows · Linux. ~6–15 MB release binary.

## Quick start

Requires **Rust 1.78+**, **Node 20+**, and **pnpm**.

```sh
git clone <repo-url>
cd pixmill
pnpm install
pnpm tauri dev
```

On Linux you'll also need GTK / WebKit dev headers — see [Tauri's prerequisites
guide](https://tauri.app/start/prerequisites/) for the exact apt/dnf incantations.

The `webp` crate uses `bindgen` at build time, which needs `libclang`. macOS
already provides it via the Xcode Command Line Tools. On Linux, install
`libclang-dev`. On Windows, install [LLVM](https://releases.llvm.org/) (e.g.
`winget install LLVM.LLVM`) if it isn't already on `PATH` via the Visual Studio
C++ workload.

## Build a release bundle

```sh
pnpm tauri build
```

This produces a `.dmg` on macOS, a `.msi` on Windows, and `.AppImage` + `.deb`
on Linux. Bundles land in `src-tauri/target/release/bundle/`.

## Web build

The same image pipeline runs in the browser as a SvelteKit SPA backed by a
wasm-compiled `pixmill-core`. Output goes to a downloadable ZIP instead of a
folder you pick.

```sh
pnpm build:wasm   # build pixmill-core into JS bindings via wasm-pack
pnpm build:web    # build:wasm + VITE_PLATFORM=web vite build → ./build
```

`pnpm build:web` needs the Rust toolchain (pinned by `rust-toolchain.toml`),
the `wasm32-unknown-unknown` target, and `wasm-pack` on `PATH`. Install with
`cargo install wasm-pack`. The first wasm-pack run downloads `wasm-opt` from
`github.com/WebAssembly/binaryen/releases` and caches it under
`~/.cache/.wasm-pack/`.

Deployment runs on **GitHub Pages** via
[`.github/workflows/deploy.yml`](.github/workflows/deploy.yml). Every push to
`main` builds the wasm + frontend on GitHub Actions and publishes `build/`
to Pages. PRs run the same build as a check but don't deploy — GH Pages
hosts only one site per repo. The workflow sets `BASE_PATH=/<repo>` so
SvelteKit's absolute URLs resolve under `https://<user>.github.io/<repo>/`;
binding a custom domain lets you drop the prefix (set the workflow's
`BASE_PATH` env to an empty string in that case).

## Architecture

A small Tauri shell drives a Rust image-processing crate.

```
pixmill/
├── crates/pixmill-core/      Rust image pipeline (testable, no Tauri deps)
├── src-tauri/                Tauri shell: IPC commands, persistence, dialogs
└── src/                      SvelteKit frontend (SPA mode, Svelte 5 runes)
```

The split means the pipeline can be unit-tested without spinning up a window.
The Tauri side stays thin: it converts IPC types, calls into `pixmill-core`, and
streams progress back to the UI via `tauri::ipc::Channel`.

## Development

```sh
pnpm tauri dev            # run the app with hot reload
pnpm check                # type-check the frontend
cargo test -p pixmill-core    # 21 image-pipeline tests
cargo check -p pixmill-core   # fast iteration without GTK system libs
```

The `pixmill-core` integration tests synthesize real PNG fixtures and run them
through the full pipeline, so they cover ingest, metadata, decode, resize,
crop, rotate, EXIF orientation, encode, and parallel batch execution end-to-end.

## AVIF and HEIC (optional)

These formats need system libraries because no mature pure-Rust decoder exists
yet. They're gated behind Cargo features:

| Platform | Install                                     | Then build with                                                       |
| -------- | ------------------------------------------- | --------------------------------------------------------------------- |
| macOS    | `brew install dav1d libheif`                | `pnpm tauri build -- --features "pixmill-core/avif-decode pixmill-core/heic"` |
| Linux    | `sudo apt install libdav1d-dev libheif-dev` | same as above                                                         |
| Windows  | `vcpkg install dav1d libheif`               | same as above                                                         |

AVIF and HEIC inputs are auto-converted to JPEG on output (HEIC encoders are a
mess — see [`PLAN.md`](./PLAN.md) for the design note).

## Roadmap

The MVP is shipped. Planned follow-ups, roughly in order:

1. Target-file-size resize (iterative quality search)
2. Full EXIF blob copy on output (today: orientation only)
3. Watch-folder mode
4. Named presets

The full backlog with implementation notes is in [`PLAN.md`](./PLAN.md).

## Contributing

[`CLAUDE.md`](./CLAUDE.md) is the canonical guide for working in this repo — it
covers conventions, where logic should live, and the things that will trip you
up (AVIF/HEIC feature flags, the TS↔Rust type mirroring). Read it before
opening a PR.

## License

GNU Affero General Public License v3.0 or later — see [`LICENSE`](./LICENSE)
for the full text. The AGPL's network clause means that if you run a modified
version of Pixmill as a service over a network, you have to make your changes
available to its users.
