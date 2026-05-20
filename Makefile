.PHONY: help check lint fmt fmt-check type-check test dev build build-frontend

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "  check          Run all CI checks (fmt-check + lint + type-check + test)"
	@echo "  lint           ESLint + Cargo clippy"
	@echo "  fmt            Auto-format frontend (Prettier) and Rust (cargo fmt)"
	@echo "  fmt-check      Check formatting without modifying files"
	@echo "  type-check     svelte-check + cargo check -p ibp-core"
	@echo "  test           cargo test -p ibp-core"
	@echo "  dev            pnpm tauri dev"
	@echo "  build          pnpm tauri build (release bundle)"
	@echo "  build-frontend pnpm build (static frontend only)"

check: fmt-check lint type-check test

lint:
	pnpm lint
	cargo clippy --workspace -- -D warnings

fmt:
	pnpm format
	cargo fmt --all

fmt-check:
	pnpm format:check
	cargo fmt --all -- --check

type-check:
	pnpm check
	cargo check -p ibp-core

test:
	cargo test -p ibp-core

dev:
	pnpm tauri dev

build:
	pnpm tauri build

build-frontend:
	pnpm build
