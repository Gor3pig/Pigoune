#!/usr/bin/env bash
set -euo pipefail

dnf install -y --setopt=install_weak_deps=False \
    git gcc pkgconf-pkg-config gtk4-devel libadwaita-devel libseccomp-devel \
    lcms2-devel fontconfig-devel blueprint-compiler gettext glib2-devel \
    desktop-file-utils appstream glycin-loaders bubblewrap dejavu-sans-fonts ghostscript

git config --system --add safe.directory '*'
git clone --quiet /src /work

useradd --create-home builder
export CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rustup
mkdir -p "$CARGO_HOME" "$RUSTUP_HOME"
chown -R builder /work "$CARGO_HOME" "$RUSTUP_HOME"

su builder --command '
    set -euo pipefail
    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain none
    source "$CARGO_HOME/env"
    cd /work
    set +e; command -v gs; gs --version; id; echo $PATH; F=/work/crates/pigoune-core/tests/fixtures/pages-cairo.pdf
    ls -ld /work /work/crates /work/crates/pigoune-core/tests/fixtures $F
    echo A; gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -sOutputFile=/tmp/a.png $F; echo rcA=$?
    echo B; gs -q -dBATCH -dNOPAUSE -sDEVICE=png16m -sOutputFile=/tmp/b.png $F; echo rcB=$?
    cp $F /tmp/c.pdf; echo C; gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -sOutputFile=/tmp/c.png /tmp/c.pdf; echo rcC=$?
    echo D; cd /work/crates/pigoune-core/tests/fixtures && gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -sOutputFile=/tmp/d.png ./pages-cairo.pdf; echo rcD=$?
    echo E; gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -sOutputFile=/tmp/e.png -dFirstPage=1 -dLastPage=1 -dPDFFitPage -g100x100 $F; echo rcE=$?
'
