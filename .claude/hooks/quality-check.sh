#!/bin/bash
PROJECT_DIR="/Users/thomas.schmitt/dev/agentic-coding-foundation/image-batch-processor"
cd "$PROJECT_DIR"

failed=""

pnpm lint 2>&1 || failed="${failed} pnpm lint"
pnpm format:check 2>&1 || failed="${failed} pnpm format:check"
cargo fmt --check -p ibp-core 2>&1 || failed="${failed} cargo fmt --check -p ibp-core"
cargo clippy -p ibp-core -- -D warnings 2>&1 || failed="${failed} cargo clippy -p ibp-core"

if [ -n "$failed" ]; then
  printf '{"continue":false,"stopReason":"Fix these checks before finishing:%s"}' "$failed"
fi
