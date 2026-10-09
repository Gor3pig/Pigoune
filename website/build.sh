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

to_thumbnail() {
    if command -v cwebp >/dev/null; then
        cwebp -quiet -q 80 -m 6 -resize 320 0 "$1" -o "$2"
    else
        magick "$1" -resize 320x -quality 80 -define webp:method=6 "$2"
    fi
}

rm -rf "$output"
mkdir -p "$output/images"
cp -r "$repository"/website/{index.html,404.html,sitemap.xml,style.css,script.js,language.js,pigoune.flatpakref,pigoune.flatpakrepo,pigoune.gpg,fr} "$output/"
cp "$repository/data/icons/pigoune-64x64.png" "$repository/data/icons/pigoune-256x256.png" "$output/images/"
cp "$repository/data/screenshots/01.png" "$output/images/share.png"
for screenshot in "$repository"/data/screenshots/[0-9][0-9].png; do
    to_webp "$screenshot" "$output/images/$(basename "$screenshot" .png).webp"
done
for number in 02 03 04 05 06 07 11; do
    to_thumbnail "$repository/data/screenshots/$number.png" "$output/images/thumb-$number.webp"
done
to_webp "$repository/website/wallpaper-desktop.png" "$output/images/wallpaper-desktop.webp"
