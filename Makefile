.PHONY: help check install hooks clean lint fmt fmt-check type-check test dev build build-frontend build-wasm

# wasm-pack output that svelte-check (via worker.ts's static import) and
# `pnpm build` both need. Gitignored; rebuilt when wasm or core Rust changes.
WASM_PKG := src/lib/wasm/pkg
WASM_SRCS := $(shell find crates/pixmill-wasm/src crates/pixmill-core/src -name '*.rs' 2>/dev/null)

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "  install        pnpm install (auto-run by other targets when needed)"
	@echo "  hooks          Enable repo-local git hooks (.githooks/) for this clone"
	@echo "  check          Run all CI checks (fmt-check + lint + type-check + test)"
	@echo "  lint           ESLint + Cargo clippy"
	@echo "  fmt            Auto-format frontend (Prettier) and Rust (cargo fmt)"
	@echo "  fmt-check      Check formatting without modifying files"
	@echo "  type-check     svelte-check + cargo check -p pixmill-core"
	@echo "  test           cargo test -p pixmill-core"
	@echo "  build-wasm     wasm-pack build crates/pixmill-wasm (regenerates $(WASM_PKG))"
	@echo "  dev            pnpm tauri dev"
	@echo "  build          pnpm tauri build (release bundle)"
	@echo "  build-frontend pnpm build (static frontend only)"
	@echo "  clean          Remove Rust + frontend build artifacts (keeps node_modules)"

# Re-install when package.json or the lockfile is newer than node_modules.
# `touch` updates node_modules' own mtime since pnpm mutates contents without it.
node_modules: package.json pnpm-lock.yaml
	pnpm install
	@touch node_modules

# Rebuild wasm-pack output when Rust sources or workspace manifests change.
# `@touch` updates the directory mtime since wasm-pack rewrites contents
# without changing the parent dir. We bootstrap wasm-pack via cargo on
# fresh clones so `make check` / the pre-commit hook works without an
# undocumented prereq step; subsequent runs find it on PATH and skip.
$(WASM_PKG): $(WASM_SRCS) crates/pixmill-wasm/Cargo.toml crates/pixmill-core/Cargo.toml Cargo.toml Cargo.lock
	@command -v wasm-pack >/dev/null 2>&1 || { \
		echo "wasm-pack not found — installing via cargo (one-time)..."; \
		cargo install wasm-pack --locked; \
	}
	pnpm build:wasm
	@touch $(WASM_PKG)

build-wasm: $(WASM_PKG)

install: node_modules

# core.hooksPath is per-clone (not tracked in the repo), so each clone runs this once.
hooks:
	git config core.hooksPath .githooks
	@echo "Pre-commit hook enabled — 'make check' will gate every commit."

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

type-check: node_modules $(WASM_PKG)
	pnpm check
	cargo check -p pixmill-core

test:
	cargo test -p pixmill-core

dev: node_modules $(WASM_PKG)
	pnpm tauri dev

build: node_modules $(WASM_PKG)
	pnpm tauri build

build-frontend: node_modules $(WASM_PKG)
	pnpm build

clean:
	cargo clean
	rm -rf build .svelte-kit $(WASM_PKG)
