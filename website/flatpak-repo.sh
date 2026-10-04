#!/usr/bin/env bash
set -euo pipefail

bundle=${1:?usage: website/flatpak-repo.sh BUNDLE OUTPUT_REPO SIGNING_KEY_ID}
repo=${2:?usage: website/flatpak-repo.sh BUNDLE OUTPUT_REPO SIGNING_KEY_ID}
signing_key=${3:?usage: website/flatpak-repo.sh BUNDLE OUTPUT_REPO SIGNING_KEY_ID}
signing=(--gpg-sign="$signing_key" ${GNUPGHOME:+--gpg-homedir="$GNUPGHOME"})

rm -rf "$repo"
ostree init --mode=archive-z2 --repo="$repo"
flatpak build-import-bundle "${signing[@]}" "$repo" "$bundle"
flatpak build-update-repo "${signing[@]}" --title=Pigoune --default-branch=master \
    --generate-static-deltas --prune "$repo"
