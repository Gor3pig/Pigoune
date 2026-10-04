#!/usr/bin/env bash
set -euo pipefail

repository=$(cd "$(dirname "$0")/.." && pwd)
output=${1:?usage: website/build.sh OUTPUT_DIR}

rm -rf "$output"
mkdir -p "$output/images"
cp -r "$repository/website/index.html" "$repository/website/style.css" "$repository/website/script.js" "$repository/website/fr" "$output/"
cp "$repository/data/icons/pigoune-64x64.png" "$repository/data/icons/pigoune-256x256.png" "$output/images/"
cp "$repository"/data/screenshots/0[1-5].png "$output/images/"
