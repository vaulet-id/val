#!/usr/bin/env bash
# The playground's build, on a machine that has neither of its two compilers.
#
# The page is three artifacts and only one of them is JavaScript: the language
# itself, compiled to Wasm from `crates/`, and the phone preview, which is a
# Flutter web build. Both are built here rather than taken from the repository.
# `web/public/valang.wasm` is checked in so that a fresh clone can run `npm run
# dev` without Rust — and a checked-in build is a build that goes stale the
# first time somebody edits the compiler and forgets. What is deployed is built
# from the source in this commit, so it cannot disagree with it.
#
# Both toolchains land in `.vercel/cache`, which Vercel restores between builds.
# A cold build downloads about a gigabyte and takes some minutes; a warm one
# skips both downloads. Nothing depends on the cache being there — a miss is
# slow, not wrong.
set -euo pipefail
cd "$(dirname "$0")"

CACHE="$PWD/.vercel/cache"
mkdir -p "$CACHE"

# Pinned, both of them. A toolchain that follows a channel is a build that
# changes on a day nobody committed anything.
FLUTTER_VERSION=3.44.1
RUST_VERSION=1.88.0

# ------------------------------------------------------------------ Rust

export RUSTUP_HOME="$CACHE/rustup"
export CARGO_HOME="$CACHE/cargo"
export PATH="$CARGO_HOME/bin:$PATH"

if [[ ! -x "$CARGO_HOME/bin/cargo" ]]; then
  echo "▸ installing Rust $RUST_VERSION"
  curl -fsSL https://sh.rustup.rs |
    sh -s -- -y --profile minimal --default-toolchain "$RUST_VERSION"
  rustup target add wasm32-unknown-unknown
else
  echo "▸ Rust from the build cache"
fi

echo "▸ building the compiler for the browser"
./build-wasm.sh

# --------------------------------------------------------------- Flutter

export PATH="$CACHE/flutter/bin:$PATH"

if [[ ! -x "$CACHE/flutter/bin/flutter" ]]; then
  echo "▸ installing Flutter $FLUTTER_VERSION"
  curl -fsSL "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz" |
    tar -xJ -C "$CACHE"
else
  echo "▸ Flutter from the build cache"
fi

# Vercel's checkout is not the repository Flutter was unpacked from, and Flutter
# refuses to run out of a directory git calls unsafe.
git config --global --add safe.directory "$CACHE/flutter" || true
flutter --version

echo "▸ building the preview"
../preview/build.sh

# ------------------------------------------------------------------ page

echo "▸ building the page"
npm run build
