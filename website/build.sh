#!/usr/bin/env bash
set -euo pipefail

repository=$(cd "$(dirname "$0")/.." && pwd)
output=${1:?usage: website/build.sh OUTPUT_DIR}
webp_quality=85

to_webp() {
    if command -v cwebp >/dev/null; then
        cwebp -quiet -q "$webp_quality" -m 6 "$1" -o "$2"
    else
        magick "$1" -quality "$webp_quality" -define webp:method=6 "$2"
    fi
}

rm -rf "$output"
mkdir -p "$output/images"
cp -r "$repository"/website/{index.html,404.html,sitemap.xml,style.css,script.js,language.js,fr} "$output/"
cp "$repository/data/icons/pigoune-64x64.png" "$repository/data/icons/pigoune-256x256.png" "$output/images/"
cp "$repository/data/screenshots/01.png" "$output/images/share.png"
for screenshot in "$repository"/data/screenshots/0[1-5].png; do
    to_webp "$screenshot" "$output/images/$(basename "$screenshot" .png).webp"
done
