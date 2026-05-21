#!/usr/bin/env bash
# Cloudflare Pages build entry point.
#
# Pages provides Node + pnpm (via corepack) but not Rust. We install
# rustup + the wasm target + wasm-pack on demand, then defer to the
# existing `pnpm build:web` script (which chains pnpm build:wasm + vite
# build). Every step here is idempotent so Pages's per-build container
# cache can short-circuit them on repeat builds. The relevant cache
# directories — ~/.cargo, ~/.rustup, target/, and ~/.cache/.wasm-pack —
# all persist when Pages reuses a build container.

set -euo pipefail

echo "==> cf-build: installing Rust toolchain"

# Install rustup if it isn't already on PATH. `--default-toolchain none`
# defers the actual toolchain install to the rust-toolchain.toml pin
# below, which keeps the active version single-sourced.
if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain none --profile minimal --no-modify-path
fi

# shellcheck disable=SC1091
. "$HOME/.cargo/env"

# rust-toolchain.toml drives the channel + target + profile. `rustup
# show` triggers the install if the pinned toolchain isn't present yet,
# and is a no-op otherwise.
rustup show active-toolchain
rustup target add wasm32-unknown-unknown

echo "==> cf-build: installing wasm-pack"

# Building from source via `cargo install` is slower than the prebuilt
# installer but lets cargo's cache short-circuit on repeat builds and
# avoids a second network dependency. The `--locked` flag pins to the
# transitive deps wasm-pack itself ships with its Cargo.lock.
if ! command -v wasm-pack >/dev/null 2>&1; then
  cargo install wasm-pack --locked
fi

echo "==> cf-build: installing JS deps"

# Cloudflare Pages enables corepack by default but doesn't always
# activate the project's pnpm version from packageManager. Force it.
corepack enable
corepack prepare --activate

pnpm install --frozen-lockfile

echo "==> cf-build: building web bundle"

pnpm build:web

echo "==> cf-build: done"
