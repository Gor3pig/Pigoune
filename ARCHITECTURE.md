# Pigoune architecture

This document gives a map of the code base: where things live, how they fit together and
which choices shape them. It describes the current state; the code is the final reference.

## Technology

| Area | Choice |
|---|---|
| Language | Rust (edition 2024), `unsafe` forbidden |
| User interface | GTK 4 and libadwaita through `gtk4-rs` and `libadwaita-rs`, layouts written in Blueprint |
| Image decoding | glycin, which decodes images in a sandbox (SVG, raster formats including AVIF, JPEG XL and TIFF, animated GIF, ICO) |
| Data | SQLite through `rusqlite` (bundled), the single source of truth of a library |
| File fingerprints | BLAKE3 |
| Identifiers | UUID v7, ordered by creation time |
| Application settings | GSettings (`data/io.github.gor3pig.Pigoune.gschema.xml`) |
| Translations | gettext, with American English as the source language |
| Build | Cargo, driven by Meson for installation, packaged with Flatpak on the GNOME 51 runtime |

## Two crates

```
crates/
├── pigoune-core/   The library logic. No dependency on GTK.
└── pigoune-app/    The GNOME application: windows, widgets, thumbnails, drag and drop.
```

### `pigoune-core`

Everything that touches a library lives here: creating and opening libraries, the database
schema and its migrations, importing files and folders, duplicates, collections, tags,
favorites, metadata, search and filters, the trash, export and undo.

- **Commands and undo.** Every change goes through a command (`AssetCommand`,
  `CollectionCommand`, `TagCommand`). Applying a command returns its exact inverse, which the
  history keeps to undo it later. This is what makes <kbd>Ctrl</kbd>+<kbd>Z</kbd> reliable.
- **All or nothing.** Each command runs in a SQLite transaction, so a failure never leaves a
  library half changed.
- **Explicit errors.** Errors are typed (`LibraryError`, `ImportError`, `CollectionError`, ...)
  so that the application can turn each of them into a clear message.
- **Tests** live in `crates/pigoune-core/tests/` and work on real temporary libraries, with
  sample images in `tests/fixtures/`.

### `pigoune-app`

The application never decides anything on its own: it displays the library and asks the core
to change it.

- `application.rs` and `window.rs` hold the application, its actions and the main window,
  which ties the sidebar, the grid, the details panel and the preview together.
- Each widget has its own module (`sidebar.rs`, `asset_grid.rs`, `asset_details.rs`,
  `asset_preview.rs`, ...) and, when it has a fixed layout, a Blueprint file in `ui/`.
- Small pure helpers (sorting, layout math, text formatting) sit in their own modules with unit
  tests, for example `asset_sort.rs`, `grid_columns.rs`, `tag_cloud.rs` or `asset_facts.rs`.
- Long tasks (imports, thumbnails) run off the main thread or asynchronously, so that the
  interface never freezes.
- `build.rs` compiles the Blueprint files, the GResource bundle, the development translations
  and the settings schema, so that `cargo run` works without installing anything. It also
  merges the translations into the metainfo, which the About window reads to show what is new.

## The library format

A library is a self-contained folder that can be moved or copied to another computer:

```
My logos.pigoune/
├── library.db                      SQLite database: assets, collections, tags, metadata
├── files/
│   └── <uuid>/<original-name>      one folder per asset, keeping the original file name
└── cache/
    └── thumbnails/<pixels>/<uuid>.png   disposable, regenerated when missing
```

- Paths stored in the database are relative to the library, which keeps it portable.
- The database is identified by `PRAGMA application_id` and versioned by `PRAGMA
  user_version`. Migrations are listed in `crates/pigoune-core/src/library/schema.rs`: SQL
  files from `migrations/`, or a Rust step when existing files must be read again (format 2
  looks for animated PNG and WebP among assets imported earlier). A library created by a newer
  version of Pigoune is refused with a clear message.
- SQLite runs with `journal_mode=DELETE` and `synchronous=FULL`: a library is a single file at
  rest and survives power failures.
- A library is locked while it is open, so that two windows cannot write to it at once.
- The trash is a state in the database: files stay in place until the trash is emptied, which
  makes restoring exact.
- Dates are stored as Unix timestamps in milliseconds.

## Importing

1. The format is detected from the content, never from the file extension. SVG files are
   parsed without running scripts or loading external resources.
2. The original is fingerprinted. A file already in the library is never copied twice: it is
   reported, added to the target collection or restored from the trash.
3. New images are fully decoded by glycin before they are accepted, so damaged files are
   listed instead of imported.
4. The file is copied into a hidden `files/.<uuid>.partial/` folder and fingerprinted again.
5. Records are written in batches of about one second. Each batch moves its copies to their
   final folder and commits in one go. Leftover `.partial` folders are removed when a library
   is opened, so an interrupted import never leaves orphan files.

Importing a folder recreates its tree as nested collections.

## User interface notes

- **Grid layout.** GTK stretches the grid columns to fill the visible width. Each thumbnail sits
  in a square space whose height follows the width it is given (`square_space.rs`), so frames
  stay square and evenly spaced. Sizing happens during layout, never one frame late, which keeps
  resizing free of flicker.
- **Grid performance.** The grid adapts its maximum number of columns to its visible width.
  GTK keeps widgets for a number of rows times the maximum column count, and accessibility
  updates scale badly with many widgets, so a fixed high maximum makes large libraries slow.
- **Sorting** happens in the application rather than the core, to compare names the way Files
  does in the user's language.
- **Highlights** such as the "found it" glow are driven frame by frame with tick callbacks and
  plain CSS classes, so they also work when system animations are turned off.
- **Adaptive layout.** Breakpoints collapse the details panel, then the sidebar, on narrow
  windows.

## Translations

User interface strings are written in American English and extracted with gettext. See
[po/README.md](po/README.md) to add or update a translation.
