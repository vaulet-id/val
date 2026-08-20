#!/usr/bin/env bash
# Build the Flutter preview and drop it where the playground serves it from.
#
# The output is *not* checked in. It is forty megabytes of CanvasKit, and a
# repository carries its history forever — that is a permanent cost for a
# convenience, paid by everyone who ever clones this. Run this once; the
# playground says so when it has not been run.
set -euo pipefail
cd "$(dirname "$0")"
flutter build web --release --base-href /playground/preview/
rm -rf ../web/public/preview
cp -R build/web ../web/public/preview
echo "→ web/public/preview"
