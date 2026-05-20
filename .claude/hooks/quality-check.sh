#!/bin/bash
cd "/Users/thomas.schmitt/dev/agentic-coding-foundation/image-batch-processor"

if ! make check 2>&1; then
  printf '{"continue":false,"stopReason":"make check failed — fix all errors before finishing"}'
fi
