#!/usr/bin/env bash
set -euo pipefail

dnf install -y --setopt=install_weak_deps=False \
    git gcc pkgconf-pkg-config gtk4-devel libadwaita-devel libseccomp-devel \
    lcms2-devel fontconfig-devel blueprint-compiler gettext glib2-devel \
    desktop-file-utils appstream glycin-loaders bubblewrap dejavu-sans-fonts ghostscript strace

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
    for d in /home/builder /work /opt /var/tmp /tmp; do cp $F $d/z.pdf; echo "LOC $d"; gs -q -dBATCH -dNOPAUSE -sDEVICE=png16m -sOutputFile=/tmp/z.png $d/z.pdf; echo rc=$?; done
    cat /proc/self/mountinfo | grep -E " / | /work| /tmp" | head; df /work /tmp /home/builder; stat $F; lsattr $F; cat /proc/self/status | grep -i -E "seccomp|NoNewPrivs|Cap(Eff|Bnd)"
    strace -f -e trace=openat,open,access,stat,newfstatat gs -q -dBATCH -dNOPAUSE -sDEVICE=png16m -sOutputFile=/tmp/s.png $F 2>&1 | grep -E "EACCES|EPERM" | head
'
