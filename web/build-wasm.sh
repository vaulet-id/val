#!/usr/bin/env bash
# Build the compiler and runtime for the browser.
#
# The playground runs the real one. It used to carry a parser and an evaluator
# written in TypeScript — honest about being approximations, and still a second
# implementation of a language whose whole claim is that what ran can be
# checked.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p valang-web --target wasm32-unknown-unknown --release
mkdir -p web/public
cp target/wasm32-unknown-unknown/release/valang_web.wasm web/public/valang.wasm
echo "→ web/public/valang.wasm  ($(du -h web/public/valang.wasm | cut -f1))"
