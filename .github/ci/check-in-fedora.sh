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
    command -v gs; gs --version; id; echo $PATH; cargo test -p pigoune-app page_renderer::tests::draws_a_pdf -- --nocapture --test-threads=1 2>&1 | tail -30; ls -la /work/crates/pigoune-core/tests/fixtures | head -5; gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -sOutputFile=/tmp/x.png /work/crates/pigoune-core/tests/fixtures/pages-cairo.pdf; echo rc=$?; gs -q -dBATCH -dNOPAUSE -sDEVICE=png16m -sOutputFile=/tmp/y.png /work/crates/pigoune-core/tests/fixtures/pages-cairo.pdf; echo rc2=$?
'
