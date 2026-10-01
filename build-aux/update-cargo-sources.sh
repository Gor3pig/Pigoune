#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

tools_dir="${XDG_CACHE_HOME:-$HOME/.cache}/pigoune-dev"
venv_dir="$tools_dir/venv"
generator="$tools_dir/flatpak-cargo-generator.py"
generator_url="https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py"

mkdir -p "$tools_dir"
if [ ! -x "$venv_dir/bin/python" ]; then
    python3 -m venv "$venv_dir"
    "$venv_dir/bin/pip" install --quiet aiohttp tomlkit
fi
if [ ! -f "$generator" ]; then
    curl -fsSL -o "$generator" "$generator_url"
fi

"$venv_dir/bin/python" "$generator" Cargo.lock -o build-aux/cargo-sources.json
