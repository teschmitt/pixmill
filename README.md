# Pixmill

Drop a pile of photos, set resize / crop / rotate, hit Run. Cross-platform
desktop app built with Tauri 2 and a Rust image pipeline — small binary, no
Electron, no servers.

> **Status:** v1 shipped. Core pipeline (JPEG / PNG / WebP) is tested and
> working; AVIF + HEIC need optional system libs. See [`PLAN.md`](./PLAN.md)
> for the v2 backlog.

## Features

- 🗂 Drag-and-drop, file picker, recursive folder picker
- 🖼 Live thumbnail grid for up to ~100 images at a time
- 👀 Side-by-side preview — click a thumbnail to compare source vs. processed with synced zoom and pan
- ✂ Resize (long-edge px or %), crop (aspect ratio or pixels), rotate / flip
- 📦 Target-file-size compression — binary-search on encoder quality to hit a kilobyte budget
- 👁 Watch folders — live filesystem ingest with optional auto-process
- 🧭 EXIF orientation baked into output pixels — no more sideways portraits
- ⚡ Parallel batch processing in Rust (`rayon`), per-file progress over Tauri channels
- 💾 Originals are never touched — output goes to a folder you choose
- 🔁 Sticky settings — your last-used configuration restores on launch
- 🖥 Native window: macOS · Windows · Linux. ~6–15 MB release binary.
- 🌐 Same pipeline runs in the browser via WebAssembly (see [Web build](#web-build))
- 📖 Bundled in-app docs at `/help` — getting-started, operations, watch folders, preview, troubleshooting

## Quick start

Requires **Rust 1.95.0** (pinned by `rust-toolchain.toml`), **Node 20+**, and **pnpm**.

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

Deployment runs on **GitHub Pages** via a dedicated
[`.github/workflows/pages.yml`](.github/workflows/pages.yml) that triggers on
the `v*` tag push the release workflow creates — see
[Release process](#release-process). Each release rebuilds the wasm + frontend
and publishes `build/` to Pages so the hosted demo always matches the latest
tagged version. PRs run a smoke build via
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) but don't deploy.
The workflow sets `BASE_PATH=/<repo>` so SvelteKit's absolute URLs resolve
under `https://<user>.github.io/<repo>/`; binding a custom domain lets you
drop the prefix (set the workflow's `BASE_PATH` env to an empty string in
that case).

## Release process

Releases are driven by PR labels.
[`.github/workflows/release.yml`](.github/workflows/release.yml) watches for
PRs merging into `main` and, when it sees a `release:major|minor|patch` label,
bumps versions, tags, builds desktop bundles for macOS / Windows / Linux, and
publishes a GitHub Release with auto-generated notes. The tag push then
triggers [`pages.yml`](.github/workflows/pages.yml), which rebuilds the wasm +
frontend and redeploys the web demo to Pages.

**Cutting a release**: add exactly one of these labels to your PR before
merging:

| Label           | Bumps             | Use for                             |
| --------------- | ----------------- | ----------------------------------- |
| `release:major` | `1.2.3` → `2.0.0` | Breaking changes                    |
| `release:minor` | `1.2.3` → `1.3.0` | New features (backwards-compatible) |
| `release:patch` | `1.2.3` → `1.2.4` | Bug fixes, small tweaks             |

Merge with no `release:*` label and nothing happens — the change ships in
the changelog of the next labeled release. Use this for refactors, docs,
chore commits, and anything else not worth its own version.

**One-time repo setup**:

```sh
# Create the labels (run once per repo)
gh label create "release:major" --color "B60205" --description "Bumps MAJOR — breaking changes"
gh label create "release:minor" --color "0E8A16" --description "Bumps MINOR — new features"
gh label create "release:patch" --color "1D76DB" --description "Bumps PATCH — bug fixes"
gh label create "skip-changelog" --color "C2E0C6" --description "Omit from release notes"
```

If `main` has branch protection that blocks direct pushes by Actions, create
a fine-grained personal access token with `contents: write` on this repo
and save it as the `RELEASE_TOKEN` secret. The workflow falls back to the
default `GITHUB_TOKEN` when `RELEASE_TOKEN` isn't set, which works on
unprotected branches.

**What lands on `main` per release**: a single `chore: release vX.Y.Z`
commit bumping `package.json`, `Cargo.toml`, `Cargo.lock`, and prepending
the release notes to `CHANGELOG.md`. The matching annotated tag points at
that commit. `src-tauri/tauri.conf.json` reads its version from
`package.json` via Tauri 2's `"version": "../package.json"` indirection, so
it doesn't need a separate bump.

**Caveats**:

- Bundles are unsigned. macOS users see "unidentified developer" on first
  launch (right-click → Open to bypass); Windows users see a SmartScreen
  warning. Signing requires Apple Developer and Authenticode certs we don't
  have yet.
- macOS builds are universal (arm64 + x86_64), which roughly doubles the
  macOS build time per release.

## Architecture

A small Tauri shell drives a Rust image-processing crate.

```
pixmill/
├── crates/pixmill-core/      Rust image pipeline (testable, no Tauri deps)
├── crates/pixmill-wasm/      wasm-bindgen bridge exposing pixmill-core to the browser
├── src-tauri/                Tauri shell: IPC commands, persistence, dialogs, watch folders
└── src/                      SvelteKit frontend (SPA mode, Svelte 5 runes)
```

The split means the pipeline can be unit-tested without spinning up a window.
The Tauri side stays thin: it converts IPC types, calls into `pixmill-core`, and
streams progress back to the UI via `tauri::ipc::Channel`.

## Development

```sh
pnpm tauri dev            # run the app with hot reload
pnpm check                # type-check the frontend
pnpm test                 # vitest (frontend unit tests, e.g. autoBurstCoalescer)
cargo test -p pixmill-core    # 27 image-pipeline tests
cargo check -p pixmill-core   # fast iteration without GTK system libs
```

There's also a `Makefile` with shortcuts: `make check` runs the full CI suite
(fmt-check + lint + type-check + test), `make dev` is `pnpm tauri dev`, and
`make fmt` formats both the frontend and Rust side. `make help` lists the rest.

The `pixmill-core` integration tests synthesize real PNG fixtures and run them
through the full pipeline, so they cover ingest, metadata, decode, resize,
crop, rotate, EXIF orientation, encode, target-file-size compression, and
parallel batch execution end-to-end.

## AVIF and HEIC (optional)

These formats need system libraries because no mature pure-Rust decoder exists
yet. They're gated behind Cargo features:

| Platform | Install                                     | Then build with                                                               |
| -------- | ------------------------------------------- | ----------------------------------------------------------------------------- |
| macOS    | `brew install dav1d libheif`                | `pnpm tauri build -- --features "pixmill-core/avif-decode pixmill-core/heic"` |
| Linux    | `sudo apt install libdav1d-dev libheif-dev` | same as above                                                                 |
| Windows  | `vcpkg install dav1d libheif`               | same as above                                                                 |

AVIF and HEIC inputs are auto-converted to JPEG on output (HEIC encoders are a
mess — see [`PLAN.md`](./PLAN.md) for the design note).

## Roadmap

The MVP is shipped, and the v1 backlog (target-file-size compression, watch
folders, side-by-side preview, web build) is done. Planned follow-ups, roughly
in order:

1. Strip-EXIF option (privacy — currently `preserve_exif` only controls orientation)
2. Full EXIF blob copy on output (today: orientation only)
3. Named presets

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
