.PHONY: help check install lint fmt fmt-check type-check test dev build build-frontend

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "  install        pnpm install (auto-run by other targets when needed)"
	@echo "  check          Run all CI checks (fmt-check + lint + type-check + test)"
	@echo "  lint           ESLint + Cargo clippy"
	@echo "  fmt            Auto-format frontend (Prettier) and Rust (cargo fmt)"
	@echo "  fmt-check      Check formatting without modifying files"
	@echo "  type-check     svelte-check + cargo check -p pixmill-core"
	@echo "  test           cargo test -p pixmill-core"
	@echo "  dev            pnpm tauri dev"
	@echo "  build          pnpm tauri build (release bundle)"
	@echo "  build-frontend pnpm build (static frontend only)"

# Re-install when package.json or the lockfile is newer than node_modules.
# `touch` updates node_modules' own mtime since pnpm mutates contents without it.
node_modules: package.json pnpm-lock.yaml
	pnpm install
	@touch node_modules

install: node_modules

check: fmt-check lint type-check test

lint: node_modules
	pnpm lint
	cargo clippy --workspace -- -D warnings

fmt: node_modules
	pnpm format
	cargo fmt --all

fmt-check: node_modules
	pnpm format:check
	cargo fmt --all -- --check

type-check: node_modules
	pnpm check
	cargo check -p pixmill-core

test:
	cargo test -p pixmill-core

dev: node_modules
	pnpm tauri dev

build: node_modules
	pnpm tauri build

build-frontend: node_modules
	pnpm build
