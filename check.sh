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

validate_desktop_file() {
    local workdir
    workdir=$(mktemp -d)
    cp data/io.github.gor3pig.Pigoune.desktop.in "$workdir/io.github.gor3pig.Pigoune.desktop"
    desktop-file-validate "$workdir/io.github.gor3pig.Pigoune.desktop"
    local status=$?
    rm -rf "$workdir"
    return $status
}

validate_metainfo() {
    local workdir
    workdir=$(mktemp -d)
    cp data/io.github.gor3pig.Pigoune.metainfo.xml.in "$workdir/io.github.gor3pig.Pigoune.metainfo.xml"
    appstreamcli validate --no-net --explain "$workdir/io.github.gor3pig.Pigoune.metainfo.xml"
    local status=$?
    rm -rf "$workdir"
    return $status
}

translatable_files_are_listed() {
    local missing=0 file
    while IFS= read -r file; do
        if ! grep -qxF "$file" po/POTFILES.in; then
            printf 'Absent de po/POTFILES.in : %s\n' "$file"
            missing=1
        fi
    done < <(git ls-files --cached --others --exclude-standard -- '*.blp' '*.rs' \
        | xargs -r grep -lE '(^|[^A-Za-z_])(_|C_|gettext|pgettext|ngettext)\(' \
        | grep -v '/build\.rs$')
    return $missing
}

translations_are_complete() {
    local workdir language statistics incomplete=0
    workdir=$(mktemp -d)
    xgettext --files-from=po/POTFILES.in --from-code=UTF-8 \
        --keyword=_ --keyword=C_:1c,2 --keyword=gettext --keyword=pgettext:1c,2 \
        --keyword=ngettext:1,2 --package-name=pigoune \
        --output="$workdir/pigoune.pot" 2>/dev/null
    for language in $(cat po/LINGUAS); do
        msgmerge --quiet --no-fuzzy-matching "po/$language.po" "$workdir/pigoune.pot" \
            --output-file="$workdir/$language.po"
        statistics=$(msgfmt --check --statistics --output-file=/dev/null "$workdir/$language.po" 2>&1)
        printf '%s : %s\n' "$language" "$statistics"
        if grep -qE 'non traduit|untranslated|approximati|fuzzy' <<<"$statistics"; then
            incomplete=1
        fi
    done
    rm -rf "$workdir"
    return $incomplete
}

step "Formatage (rustfmt)" cargo fmt --all --check
step "Relecture stricte (clippy)" cargo clippy --workspace --all-targets --quiet -- -D warnings
step "Tests" cargo test --workspace --quiet
step "Aucun commentaire dans le code" no_comments_in_sources
step "Lanceur GNOME (.desktop)" validate_desktop_file
step "Fiche de l'application (metainfo)" validate_metainfo
step "Fichiers traduisibles déclarés" translatable_files_are_listed
step "Traductions complètes" translations_are_complete

printf '\n'
if [ ${#failures[@]} -eq 0 ]; then
    printf '\033[1;32m✅ Tout est vert.\033[0m\n'
else
    printf '\033[1;31m❌ À corriger : %s\033[0m\n' "${failures[*]}"
    exit 1
fi
