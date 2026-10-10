#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

tools_dir="${XDG_CACHE_HOME:-$HOME/.cache}/pigoune-dev"
venv_dir="$tools_dir/venv"
generator="$tools_dir/flatpak-cargo-generator.py"
generator_commit="41c20aa10819cdb2a4f3ca171758a96d1955c018"
generator_sha256="0a2db6be87d75910facef28ab46d4d6460802e8419ab850d0caa6a364d26b380"
generator_url="https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/$generator_commit/cargo/flatpak-cargo-generator.py"

mkdir -p "$tools_dir"
if [ ! -x "$venv_dir/bin/python" ]; then
    python3 -m venv "$venv_dir"
    "$venv_dir/bin/pip" install --quiet aiohttp tomlkit
fi
if [ ! -f "$generator" ]; then
    curl -fsSL -o "$generator" "$generator_url"
fi
if ! echo "$generator_sha256  $generator" | sha256sum --check --quiet; then
    rm -f "$generator"
    echo "The Flatpak cargo generator does not match the pinned version." >&2
    exit 1
fi

"$venv_dir/bin/python" "$generator" Cargo.lock -o build-aux/cargo-sources.json
