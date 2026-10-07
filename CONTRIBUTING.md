# Contributing to Pigoune

Thank you for your interest in Pigoune! Bug reports, ideas, translations and code are all
welcome. This guide explains how to get a working setup and what a good contribution looks
like.

Please read the [Code of Conduct](CODE_OF_CONDUCT.md) before taking part.

## Before you start

- **Bugs**: open an issue with the bug report template. Steps to reproduce and the Pigoune
  version make a huge difference.
- **Features**: open an issue to discuss the idea first. Pigoune aims to stay a focused, local
  library for graphic assets, so a short discussion avoids wasted work.
- **Small fixes** (typos, obvious bugs): feel free to open a pull request directly.

## Getting the code

```sh
git clone https://github.com/Gor3pig/Pigoune.git
cd Pigoune
```

## Building and running

Pigoune is written in Rust with GTK 4 and libadwaita. There are two ways to build it.

### With Flatpak (closest to what users get)

The GNOME 51 SDK and the Rust SDK extension provide every dependency, and
`flatpak-builder` installs them from Flathub if they are missing.

```sh
flatpak-builder --user --install-deps-from=flathub --install --force-clean \
    build-dir build-aux/io.github.gor3pig.Pigoune.json
flatpak run io.github.gor3pig.Pigoune
```

If you change `Cargo.lock`, regenerate the offline sources used by the Flatpak build:

```sh
./build-aux/update-cargo-sources.sh
```

GNOME Builder can also open the project and use this manifest directly.

### Natively (fastest for day-to-day work)

You need:

- Rust through [rustup](https://rustup.rs): the version used by the project is pinned in
  `rust-toolchain.toml`, and rustup installs it automatically
- GTK 4.22 or later and libadwaita 1.9 or later, with their development files
- glycin loaders and `bubblewrap` (sandboxed image decoding)
- `blueprint-compiler` (user interface files)
- gettext (`msgfmt`, `xgettext`, `msgmerge`) and GLib tools (`glib-compile-schemas`)
- For `check.sh`: `desktop-file-validate` (desktop-file-utils) and `appstreamcli` (AppStream)
- Optionally Meson 1.5 or later, which is what the Flatpak build uses

On Fedora, for example:

```sh
sudo dnf install gcc gtk4-devel libadwaita-devel libseccomp-devel lcms2-devel \
    fontconfig-devel blueprint-compiler gettext glib2-devel glycin-loaders bubblewrap \
    desktop-file-utils appstream
```

Then:

```sh
cargo run -p pigoune-app
```

During development, settings and translations are compiled next to the binary, so nothing
needs to be installed system-wide.

To build the way Flatpak does:

```sh
meson setup _build
meson compile -C _build
```

## Checking your work

Run the quality gate before every commit:

```sh
./check.sh
```

It checks formatting (`rustfmt`), strict lints (`clippy`), the test suite, the project rules
below, the desktop entry, the AppStream metainfo and the translations. A pull request is ready
when `./check.sh` is all green.

To run only the tests of the core library:

```sh
cargo test -p pigoune-core
```

If you change how a change is recorded or undone, `random_undo` is the test to watch: it replays
the same random sequences every time, and a failure names the seed to investigate.

## Project rules

These rules keep the code base consistent. `check.sh` enforces most of them.

- **No comments in the code.** Prefer clear names, short functions and modules with a single
  purpose. If something needs explaining, it usually needs a better name or a smaller function.
- **Plain hyphens only.** Use `-`, never long dashes, in code, user interface text and
  documentation.
- **The core holds the rules.** `pigoune-core` knows nothing about GTK. Every change to a
  library goes through it and is covered by tests. The application only displays and asks.
- **Filters are shared.** A new filter goes into `AssetFilter`, `filter_choices.rs` and the
  `smart_collections` table together, so that the Filters popover and smart collections stay
  identical.
- **Every user interface string is translatable**, written in American English, and every file
  that contains translatable strings is listed in `po/POTFILES.in`. If you add or change
  strings, update the French translation too (see [po/README.md](po/README.md)), or say in
  your pull request that you need help with it.
- **No panics.** Errors are explicit types, and every failure ends up as a clear message in the
  interface. `unwrap()` is only used in tests.
- **Follow the GNOME Human Interface Guidelines** for anything visible.
- **Icons** in `data/icons/` are the official artwork and are not modified.

See [ARCHITECTURE.md](ARCHITECTURE.md) for how the code is organized.

## Commit messages

Write commit messages in English:

- A short summary in the imperative mood, without a final period, ideally under 65
  characters: `Add a details panel`, `Fix thumbnails of animated GIFs`.
- An optional body, separated by a blank line and wrapped at 72 characters, explaining what
  changed and why.
- A prefix such as `core:` or `ui:` is welcome when it helps, but not required.

Keep each commit focused on one change, and make sure the project builds and passes
`./check.sh` at every commit.

## Submitting a pull request

1. Fork the repository and create a branch from `main`.
2. Make your change, with tests when it touches `pigoune-core`.
3. Run `./check.sh`.
4. Open a pull request and fill in the template. Screenshots are very helpful for visual
   changes.

The automatic check runs on every pull request. Reviews may ask for changes: this is a normal
part of the process, not a judgment of your work.

## The website

The website at <https://gor3pig.github.io/Pigoune/> lives in `website/`: one page in English
(`index.html`) and one in French (`fr/index.html`), kept identical, with a shared stylesheet and
a small script. It uses plain HTML, CSS and JavaScript, with no framework, no external fonts and
no tracking, and it works without JavaScript. The icon and the screenshots are taken from
`data/` when the site is assembled, the screenshots being converted to WebP, which needs
`cwebp` or ImageMagick. To preview it:

```sh
website/build.sh /tmp/pigoune-website
python3 -m http.server --directory /tmp/pigoune-website
```

Then open <http://localhost:8000/>. The `Website` workflow publishes it on GitHub Pages
whenever `website/`, the icons or the screenshots change on `main`, and when a release is
published, together with the signed Flatpak repository described in
[RELEASING.md](RELEASING.md).

## Releases

Releases are made by the maintainer, following [RELEASING.md](RELEASING.md).

## License

By contributing, you agree that your contributions are licensed under the
[GNU General Public License v3.0 or later](COPYING), like the rest of Pigoune.
