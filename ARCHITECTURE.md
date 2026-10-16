# Pigoune architecture

This document gives a map of the code base: where things live, how they fit together and
which choices shape them. It describes the current state; the code is the final reference.

## Technology

| Area | Choice |
|---|---|
| Language | Rust (edition 2024), `unsafe` forbidden |
| User interface | GTK 4 and libadwaita through `gtk4-rs` and `libadwaita-rs`, layouts written in Blueprint |
| Image decoding | glycin, which decodes images in a sandbox (SVG, raster formats including AVIF, HEIC, JPEG XL and TIFF, animated GIF, ICO) |
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
- The one exception is the icon choice for collections: its 294 icons are the bold weight of
  [Phosphor](https://phosphoricons.com) (MIT, license in `crates/pigoune-app/icons/`), copied
  unmodified into `crates/pigoune-app/icons/scalable/actions/` and embedded through
  `resources.gresource.xml`. `collection_looks.rs` lists them by theme; a test checks that each
  one is embedded. The gray default folder stays the Adwaita one. Phosphor drawings leave about
  6 % of margin on each side, while Adwaita icons fill their whole 16 px box, so
  `show_icon` in `collection_look_dialog.rs` displays the collection icons at 18 px (class
  `phosphor-icon`) inside a fixed 18 px slot, which gives both families the same visual size
  and keeps the labels of the sidebar aligned.
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
- **Annotation editor.** `editor.rs` is a page of the main window (the `editor` page of its stack, like the preview), reachable only inside an open library. One panel holds the tools (`editor_tool.rs`: a named grid with the icon, the label and the identifier of each tool), the settings of the active tool, the layers and the frame: it sits on the right of the canvas (`PigouneZoomView`, shared with the preview), as a full-height pane with its own header bar and the same width and background as the details pane of the library, and, in a narrow window, becomes a bottom sheet raised from a bar that recalls the tool (the save and output buttons then move into the single header bar). The library actions are disabled while it is shown. The tools are described by data, so that a tool or an option is cheap to add and old documents keep working: `annotations/tools.rs` lists, for each tool, its kind, its geometry (two points, box, polyline, anchor), its outline (for the clicks) and its options, and each option (`annotations/options.rs`) has a kind (color, opacity, number in a range, choice in a list, toggle), a default and, optionally, the toggle that shows it. A shape (`annotations/shape.rs`) is a kind, a geometry and a free dictionary of options: a missing or invalid option reads as its default, an option or a field of a newer version is kept untouched, and the document reads the same as before. The panel of settings is built from the sheet of the tool (`editor_options.rs`), the drawing is a table of renderers by kind (`annotation_render.rs`), and the last options used with each tool are remembered. Tests make sure that every tool has a button, a renderer, a label for each option and for each choice, and that every default is valid. The editing logic lives in the core, without any interface, so it is tested automatically: `annotations::EditSession` holds the document, the selection and the undo and redo history (a whole drag or slider gesture is one change), `annotations::geometry` the shapes (drawing from a drag, hit testing with a tolerance, handles, moving and resizing, the arrow head, the dash pattern), and `annotations::Interaction` turns pointer events and the active tool into those edits (a freehand stroke gathers its points while the pointer moves, then is simplified with a small tolerance when released, and an anchored badge follows the pointer until it is released, and text is typed in a real text field laid over the canvas, placed and sized from the zoom, so that accents, composed characters and several lines work like anywhere else; the core never measures text itself, the interface gives the session a measurer based on the same font layout as the drawing, with a rough estimate as the fallback for tests). The number of a numbered step is never stored: it is the rank of the step among the steps of the document, so deleting one renumbers the others and tells the cursor to show, and `annotations::CropEditor` does the same for the crop frame (clamped to the image, a minimum side, handles, moving, and fixed ratios that survive every drag). Blur and pixelation are objects that mask everything drawn before them, the image and the earlier annotations (the interface replays that scene under the mask; a pixelation is a reduced copy of the scene enlarged without smoothing, kept in a small cache), and an annotation drawn after a mask stays sharp. As long as the original image is untouched the masks can be undone, so the editor shows a banner while a mask is not locked; locking, after a confirmation, applies every mask to the original image only (tile by tile, each tile being rendered and read back at 2048 pixels at most), keeps the mask objects in the document, and remembers the locked masks so that moving or changing one brings the banner back. The final image (`annotation_export.rs`) replays the same drawing on the untouched image: the crop picks the region, and the result is rendered and read back tile by tile (`tiled_render.rs`, 2048 pixels at most, shared with the locking of the masks) so that large captures work; the output button of the editor copies it to the clipboard or exports it as a PNG through the file chooser of the system. Saving sends the final image, the untouched image of the editor (the locked masks included, always re-encoded as PNG) and the document to the window, which stores them with the annotated storage of the library under a dated name, tags the new asset "capture", shows it selected in the grid and refuses an image that is already stored by naming the existing asset; leaving the editor with unsaved changes asks for a confirmation. A capture that is being edited survives a crash or a closed window: shortly after each change the editor writes a draft in the cache folder (`capture_draft.rs`: the untouched image as a PNG and the document, the document being written to a temporary file first and then renamed), the window flushes it when it is closed during an edit, and the next time a library is shown Pigoune offers to resume it (the restored session counts as unsaved) or to discard it; saving, discarding and leaving without changes clear it. An annotated asset is an ordinary asset for everything else (grid, thumbnails, duplicates, export, trash), since its stored file is the rendered image; the details panel and the context menu add "Annotate…" (reopens the original with its document in the editor, and saving updates the same asset: new rendering, new document and, through `Library::replace_original`, the original of the editor, so that locked masks stay locked) and "View Original" (opens a copy of the untouched image in the default application). Editing an existing asset writes no draft, because the asset itself is never changed before saving. "Paste and Annotate" (Ctrl+Shift+V by default, in the import menu, a customizable shortcut) reads the clipboard like the paste of the library, takes the image (or the only file that is an image) and opens it in the editor, refusing images of more than 8000 pixels on a side; the editor opens an image from a file or from a texture alike. Taking a screenshot (`screenshot.rs`) asks the Screenshot portal for an interactive capture over the session bus, as the wallpaper code does with its portal: the window hides while the portal chooses the area, the answer (a file URI, a cancellation or a failure) is waited for through a channel, the file is read and removed, and the image goes to the editor like a pasted one; no Flatpak permission is needed. The same request is reachable from the import menu and a customizable shortcut (Ctrl+Shift+S), from the command line (`pigoune --capture`, forwarded to the running instance or served by a fresh one that reopens the last library whatever the preference says) and from the "Take a Screenshot" action of the launcher icon, which runs that command; a desktop that binds a global shortcut to the command gets the same behavior. Holding Shift while drawing constrains the shape (`geometry::constrained_end`, applied by `Interaction` when it was told so by the editor, which reads the modifiers from the drag gesture of the canvas): a box becomes a square or a circle, a segment snaps to the multiples of 45 degrees. Each tool has a one-letter shortcut (`EditorTool::shortcut`, shown in its tooltip) handled by the editor when no text is being typed and no Ctrl, Alt or Super key is held. The spotlight tool dims everything but a rectangle or an oval (a veil filled with the even-odd rule over the whole image, so it needs the scene of the drawing to know the image size); it hides nothing, so it is not a mask. Arrows can be curved and tapered with two options of their sheet: `curve` bends the shaft as a quadratic curve whose control point is offset from the middle of the segment (the head follows the tangent, clicks and bounds follow the curve) and `shaft` makes it thin at the tail and thick at the head. Three more styles come from the sheets: a numbered step can count in letters (`step_label`, A to Z then AA, always derived from the rank), a text can sit on a bubble (the color of the text becomes the bubble and the letters take a contrasting ink) and carry an outline (the layout drawn a few times around, in the contrasting ink), and every drawn tool (rectangle, ellipse, line, arrow, pen, step, text) has a shadow toggle that wraps its drawing in a shadow node scaled to the image width; masks, spotlight and highlighter have none. Every color row of the panel ends with an eyedropper (`OptionEvent::PickColor`): the next press on the canvas samples the pixel under the pointer by rendering it alone into a 1 by 1 texture (`color_sampler.rs`, so the whole image is never read back) and sets the color of the option that asked, through the usual path; Escape cancels. A color that is not in the palette leaves every swatch unchecked. A crop is a property of the document, not an object: the rectangle kept from the untouched original is one undoable edit of the session, and the view shows only that region. `PigouneZoomView` can show only a region of the image and gives the editor an overlay drawn in image coordinates and a pointer handler that claims a drag, so that dragging pans the image only when no tool takes it.
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
│                                        (+ original.png beside the rendering of an annotated capture)
└── cache/
    ├── visual-details.db           detailed signatures for duplicates, recomputed when missing
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
  the icon and color of each collection, stored as plain names (`heart`, `pink`) that the
  app turns into an icon and accent color, falling back to the gray folder for a name it
  does not know. The first versions stored names of GNOME icons (`emote-love`); the app keeps
  that list of old names forever and shows the matching Phosphor icon, so no library needs
  to be converted. Format 6 adds the main colors of each asset and the colors of each smart
  collection. `dominant_colors` in the core sorts the visible pixels of a thumbnail into
  thirteen color families and keeps the average color of each; format 7 analyzes every asset
  again to record these averages, and lets a smart collection save a custom color, matched
  when it is close to an average as the eye sees it (OKLab distance). The app analyzes assets
  with no colors yet in the background, one asset at a time, starting two seconds after a library
  opens and after each import, and only while the user is calm: `analysis_pace` pauses the pass
  while keys, pointer or scrolling are active, and a library change stops a pass in progress. Format 8 lets a smart collection save shapes (landscape, portrait, square) and
  "fits my screen", and turns every older smart collection into a search of the whole library
  (one limited to Favorites keeps "favorites only"); the screen size itself is never saved: the app gives the library the size
  of the monitor showing its window, in real pixels, and refreshes when it changes. Format 9
  gives each tag an optional parent: tags form a tree, a name is unique among siblings only, and
  opening a tag also shows the assets of its sub-tags; existing tags become top-level tags. Format 10
  adds a visual fingerprint of 64 bits to each asset, used to find look-alike assets; `visual_hash` in
  the core computes it from the pixels of a thumbnail (plain background and transparent margins
  ignored, then a 9 by 8 grid of brightness steps), and two assets are close when few bits differ.
  Format 11 added a detailed signature of 16 by 16 colored cells (format 12 moves it out of the assets table into `cache/visual-details.db`, a derived file attached to the database connection: it keeps the main database small, is left out of the main integrity check but has its own check when the library opens, is made again if it is damaged, never blocks a deletion, and is computed again on demand if it is lost) to each asset, computed for a new asset in the background pass and, for an asset that only has a hash, once when a duplicate search needs it (candidates only, with a progress bar); the hash only proposes candidates, and two assets are offered as possible duplicates only when the hash, the proportions and the detailed signatures all agree, in groups where every pair agrees. Assets are only ever moved to the trash from an explicit, confirmed selection that the core checks again at that moment. Format 12 also keeps what the user ignored as actions, kept in the main database because they are decisions, not a cache: `ignored_duplicate_actions` (a group or one asset, with its date) and `ignored_duplicate_members` (the assets of the action, with a cascading delete and a `kept_apart` flag for the look-alikes the action keeps away from the group). Ignoring a group lists only the assets of that group, and keeps them apart from the rest of the cluster of assets that still look alike (those other assets are flagged `kept_apart`, stay out of the entry, and are still grouped among themselves); ignoring one asset keeps it apart from that whole cluster. A group action stays (and stays listed) while at least one of its assets exists and either a second one remains or it still keeps other assets apart; it is forgotten only when nothing is left to hide. A pair is ignored when one action covers it, so showing one action again never touches another. Ignoring and showing again go through the history as `Change::Duplicate`, the grouping treats an ignored pair as a pair that does not match, the list of ignored duplicates has one entry per action, and a confirmed removal is refused for an ignored pair. Format 13 adds `asset_annotations` for annotated captures: the asset keeps the annotated image as its stored file (so the grid, the preview, the duplicates, the exports and the trash work unchanged), the untouched capture sits beside it as `original.png`, and the table holds the annotation document (versioned JSON, described by `annotations.rs`: an optional crop and a list of objects whose order is the layer order, each object being a tool kind with its geometry and its options (rectangle, ellipse, line, arrow, blur or pixelation for now); an object of a tool this version does not know, and any field it does not know, are kept and written back untouched, so a newer version's annotations are never destroyed by an older one, while a document of a newer version is refused with a clear message and the image stays usable), whether it hides anything with a blur, the size of the original and the date of the last change. Saving an annotated image goes through the normal import steps and is refused, naming the existing asset, when the same image is already stored; updating one swaps the rendering and the document in one transaction and leaves the original alone.
  A library created by a newer version of Pigoune is refused with a clear message.
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
