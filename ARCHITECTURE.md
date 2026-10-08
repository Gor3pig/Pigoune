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
schema and its migrations, importing files and folders, duplicates, collections, smart
collections, tags, favorites, metadata, search and filters, the trash, export and undo.

- **Commands and undo.** Every change goes through a command (`AssetCommand`,
  `CollectionCommand`, `TagCommand`, `SmartCollectionCommand`). Applying a command returns its exact inverse, which the
  history keeps to undo it later. This is what makes <kbd>Ctrl</kbd>+<kbd>Z</kbd> reliable.
- **All or nothing.** Each command runs in a SQLite transaction, so a failure never leaves a
  library half changed.
- **Explicit errors.** Errors are typed (`LibraryError`, `ImportError`, `CollectionError`, ...)
  so that the application can turn each of them into a clear message.
- **Smart collections** are saved searches over the whole library, the trash left aside: an
  `AssetFilter` (words, formats, shapes, colors, fitting the screen, favorites only). Their
  content is computed when they are shown, so new imports appear by themselves. `AssetView::Smart` lets the rest of the code treat them like any other view.
- **Tests** live in `crates/pigoune-core/tests/` and work on real temporary libraries, with
  sample images in `tests/fixtures/`. `random_undo.rs` plays seeded random sequences of changes
  (trash, favorites, renames, collections, tags, smart collections) and checks that undoing them
  all brings the library back to its exact starting state; when it fails, the message gives the
  seed to replay. `exotic_imports.rs`, `export_edge_cases.rs` and `library_failures.rs` cover
  odd files, odd names and broken libraries.

### `pigoune-app`

The application never decides anything on its own: it displays the library and asks the core
to change it.

- `application.rs` and `window.rs` hold the application, its actions and the main window,
  which ties the sidebar, the grid, the details panel and the preview together.
- Each widget has its own module (`sidebar.rs`, `asset_grid.rs`, `asset_details.rs`,
  `asset_preview.rs`, ...) and, when it has a fixed layout, a Blueprint file in `ui/`.
- **Filters are defined once.** `filter_choices.rs` builds the filter checkboxes used both by
  the Filters popover and by the smart collection window. A new filter is added there, to
  `AssetFilter` in the core, and to the `smart_collections` table, so that smart collections
  can always save every filter.
- **Search pills.** `query_pills.rs` shows a query as words joined by "and" or "or", under the
  search field and in the smart collection window. It relies on `query_groups` and
  `query_text` from the core, which also drive the search, so both always agree.
- Every symbolic icon of the interface comes from the GNOME icon theme, Adwaita, so Pigoune
  ships no icon of its own apart from the application icon in `data/icons/`. `icon_theme.rs`
  keeps Adwaita even when the system uses another icon theme, so the icons look the same on
  every distribution and match the screenshots and the user guide.
- Small pure helpers (sorting, layout math, text formatting) sit in their own modules with unit
  tests, for example `asset_sort.rs`, `grid_columns.rs`, `tag_cloud.rs` or `asset_facts.rs`.
- Inside Flatpak, folders chosen in the file chooser arrive as document portal paths
  (`/run/user/<uid>/doc/<id>/...`). `host_path.rs` asks the portal for the real location, which
  is only shown to the user: the library is still opened through the portal path.
- **Update banner.** Inside Flatpak, `flatpak_updates.rs` asks the Flatpak portal for an update
  monitor, which checks the repository Pigoune was installed from. Pigoune gets no network
  access of its own. `update_news.rs` turns the portal signals into a state (available,
  installing, installed, failed) and `update_banner.rs` words it; the banner installs the
  update through the portal, then starts the new version with the portal's `Spawn`. Outside
  Flatpak, nothing is watched.
- **What is new.** `whats_new.rs` opens a short window the first time a newer version starts:
  the last version seen is kept in the settings, and `decide` (pure, tested) tells whether to
  show it, only remember the version (first installation, preference off, a version without
  points) or do nothing. The points of the running version live in the code, so they are
  translated in every language of the interface like the rest of the texts.
- **First launch.** `first_launch.rs` is the one place that decides how Pigoune looks to a brand
  new user: a short table of settings and their first values. It is applied once, at start-up,
  and only when no setting has ever been saved; anyone who has already used or customized
  Pigoune sees no change, and the new user's later choices are never overridden. Internal state
  (last version seen, last library...) stays out of the table, and a window size is capped to
  the screen.
- **Wallpapers.** `wallpaper_dialog.rs` is the framing window and `wallpaper_stage.rs` its
  virtual screen, drawn at the real resolution of a monitor listed by `screen_size.rs`. The
  framing math (fill, whole image, zoom around a point, snapping, the visible part) is pure and
  tested in `wallpaper_framing.rs`. `desktop_panels.rs` describes the bars of each desktop as a
  short list of shapes and words, drawn by `desktop_bars.rs`. `image_conversion::wallpaper`
  makes the final image with the raster operations of the core (crop, place, blur, gradient,
  tiles, mirror, dim), then `wallpaper.rs` hands it to the Wallpaper portal.
- Long tasks (imports, thumbnails) run off the main thread or asynchronously, so that the
  interface never freezes.
- Thumbnails (`thumbnails.rs`): a tile that appears asks for its thumbnail, and at most four
  loads run at once (`load_slots.rs`), a tile that leaves the screen giving up its load. A new
  thumbnail is shrunk on the processor in a background task, `RgbaImage::shrunk_to` in the core
  averaging whole blocks before the quality filter, and its PNG is encoded in the background
  too, so scrolling a library without thumbnails does not block the window.
- The count of a single view comes from `Library::view_count`; `view_counts` is kept for the
  sidebar, where the smart collections are counted in one pass over the library.
- `build.rs` compiles the Blueprint files, the GResource bundle, the development translations
  and the settings schema, so that `cargo run` works without installing anything. It also
  merges the translations into the metainfo, which the About window reads to show what is new.

## The library format

A library is a self-contained folder that can be moved or copied to another computer:

```
My logos.pigoune/
├── library.db                      SQLite database: assets, collections, tags, metadata
├── files/
│   └── <xx>/<uuid>/<original-name>      one folder per asset, keeping the original file name
└── cache/
    └── thumbnails/<pixels>/<xx>/<uuid>.png   disposable, regenerated when missing
```

- Paths stored in the database are relative to the library, which keeps it portable.
- `<xx>` is a bucket named after the last two hexadecimal digits of the asset UUID, so that no
  folder holds more than a small share of the assets. These digits are random, unlike the
  beginning of a UUIDv7, which encodes the creation time.
- The database is identified by `PRAGMA application_id` and versioned by `PRAGMA
  user_version`. Migrations are listed in `crates/pigoune-core/src/library/schema.rs`: SQL
  files from `migrations/`, or a Rust step when existing files must be read again (format 2
  looks for animated PNG and WebP among assets imported earlier). Format 3 records the bucketed
  paths, then moves the folders and thumbnails into their buckets; every opening finishes moves
  left behind by an interruption. Format 4 adds the `smart_collections` table. Format 5 adds
  the icon and color of each collection, stored as plain names (`emote-love`, `pink`) that the
  app turns into a GNOME icon and accent color, falling back to the gray folder for a name it
  does not know. Format 6 adds the main colors of each asset and the colors of each smart
  collection. `dominant_colors` in the core sorts the visible pixels of a thumbnail into
  thirteen color families and keeps the average color of each; format 7 analyzes every asset
  again to record these averages, and lets a smart collection save a custom color, matched
  when it is close to an average as the eye sees it (OKLab distance). The app analyzes assets
  with no colors yet in the background, one batch at a time, after opening a library and after
  each import. Format 8 lets a smart collection save shapes (landscape, portrait, square) and
  "fits my screen", and turns every older smart collection into a search of the whole library
  (one limited to Favorites keeps "favorites only"); the screen size itself is never saved: the app gives the library the size
  of the monitor showing its window, in real pixels, and refreshes when it changes. Format 9
  gives each tag an optional parent: tags form a tree, a name is unique among siblings only, and
  opening a tag also shows the assets of its sub-tags; existing tags become top-level tags. A library
  created by a newer version of Pigoune is refused with a clear message.
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
- **Search field width.** `search_space.rs` holds the search field and the Filters button and
  shares the room it is given during layout: two thirds of the spare width go to the field, up
  to a maximum, and the rest stays empty so that the window can still be dragged by its header.
  Sizing during layout, like `square_space.rs`, avoids any flicker while resizing.

## Website

`website/` holds the presentation website, published on GitHub Pages by
`.github/workflows/website.yml`. `website/build.sh` assembles it with the icon and the
screenshots from `data/`, so they are never duplicated; the screenshots are converted to WebP
to keep the pages light, and the first one is kept as a PNG for link previews.
`website/wallpaper-desktop.png`, the virtual screen cut out of the wallpaper screenshot, is the
only image kept in `website/` itself.

The same workflow publishes the signed Flatpak repository under `repo/`, rebuilt each time by
`website/flatpak-repo.sh` from the package of the latest release, so it only holds that
version. `pigoune.flatpakref` installs Pigoune from it and `pigoune.flatpakrepo` adds it as a
remote. Release packages also point to this repository, so new installations update
themselves. See [RELEASING.md](RELEASING.md). The script only adds comfort (sections
fading in, enlarged screenshots, the latest version on the download button): every page works
without it, and animations are turned off for visitors who ask for reduced motion.

## Translations

User interface strings are written in American English and extracted with gettext. See
[po/README.md](po/README.md) to add or update a translation.
