#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")"

failures=()

step() {
    local label="$1"
    shift
    printf '\n\033[1;34m▶ %s\033[0m\n' "$label"
    if "$@"; then
        printf '\033[1;32m✔ %s\033[0m\n' "$label"
    else
        printf '\033[1;31m✘ %s\033[0m\n' "$label"
        failures+=("$label")
    fi
}

no_comments_in_sources() {
    local found
    found=$(git ls-files --cached --others --exclude-standard -- '*.rs' '*.blp' \
        | xargs -r grep -nE '^\s*(//|/\*)|[;,{}()]\s*(//|/\*)' || true)
    if [ -n "$found" ]; then
        printf '%s\n' "$found"
        return 1
    fi
}

step "Formatage (rustfmt)" cargo fmt --all --check
step "Relecture stricte (clippy)" cargo clippy --workspace --all-targets --quiet -- -D warnings
step "Tests" cargo test --workspace --quiet
step "Aucun commentaire dans le code" no_comments_in_sources

printf '\n'
if [ ${#failures[@]} -eq 0 ]; then
    printf '\033[1;32m✅ Tout est vert.\033[0m\n'
else
    printf '\033[1;31m❌ À corriger : %s\033[0m\n' "${failures[*]}"
    exit 1
fi
