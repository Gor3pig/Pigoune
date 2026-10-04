# Pigoune user guide

*[Version française](../fr/README.md)*

Pigoune keeps all your graphic assets (icons, logos, illustrations) in one tidy library. This
guide walks you through everything it can do.

- [Getting started](#getting-started)
- [Organizing](#organizing)
- [Finding assets](#finding-assets)
- [Looking at assets](#looking-at-assets)
- [Reusing assets](#reusing-assets)
- [Trash and undo](#trash-and-undo)
- [Your library](#your-library)
- [Preferences](#preferences)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Questions and answers](#questions-and-answers)

![The main window of Pigoune](../../data/screenshots/01.png)

## Getting started

### Create a library

A library is a folder where Pigoune keeps a copy of your assets. On the welcome screen, choose
**Create a Library**, give it a name and pick where to save it. You can create as many
libraries as you like, for example one per project or per client.

To open an existing library, choose **Open a Library**. Pigoune reopens the last library you
used every time it starts.

### Import assets

Click the **+** button at the top of the sidebar, or press <kbd>Ctrl</kbd>+<kbd>I</kbd>, and
choose **Import Files…** or **Import a Folder…**. You can also simply drag files or folders
from Files onto the Pigoune window.

- Supported formats are SVG, PNG, JPEG, WebP, AVIF, JPEG XL, GIF, TIFF, BMP
  and ICO.
- Pigoune checks every image before importing it, so damaged files are listed instead of
  added.
- Duplicates are recognized: a file already in the library is never copied twice.
- When you import a folder, its subfolders become collections.
- Your original files are never modified or moved. You can delete them once they are imported.

If a collection or a tag is selected in the sidebar, imported files go straight into it. You
can also drop files directly onto a collection or a tag in the sidebar.

## Organizing

### Collections

Collections work like folders, and can contain other collections. Create one with the **+**
button next to **Collections** in the sidebar. Right-click a collection to create a
sub-collection, rename it (<kbd>F2</kbd>) or delete it.

- **Move** assets into a collection by dragging them onto it from the grid.
- **Add** them to a collection while keeping them where they are by holding <kbd>Ctrl</kbd>
  while dragging, or with **Add to a Collection…** in the right-click menu. An asset can belong
  to several collections.
- **Reorder** collections by dragging them in the sidebar, or sort them by name or creation
  date with the sort button next to **Collections**.
- **Unclassified** lists the assets that belong to no collection.

When you delete a collection, its sub-collections are deleted too, and the assets that only
belonged to them go to the trash. Assets that also belong to another collection stay there.

### Tags

Tags describe assets with words of your choice. Add them from the details panel with the
**+** button next to the tags, or drag assets onto a tag in the sidebar. Click a tag in the
sidebar to see all its assets, or right-click it to rename or delete it. Renaming a tag to the
name of another one merges them.

### Favorites

Mark an asset as a favorite with the star next to its name in the details panel, or press
<kbd>Ctrl</kbd>+<kbd>D</kbd>. Favorites have their own entry in the sidebar.

### Names, notes and credits

Click the name of an asset in the details panel (or press <kbd>F2</kbd>) to rename it. The
**Information** list also lets you write a note and record the source, the license and the
author of each asset, which is handy when you reuse it later.

### Several assets at once

Select several assets with <kbd>Ctrl</kbd>+click, <kbd>Shift</kbd>+click, by drawing a
rectangle from an empty area of the grid, or with <kbd>Ctrl</kbd>+<kbd>A</kbd>. The details
panel then shows a summary and lets you change favorites and tags for all of them, while
dragging and the right-click menu work on the whole selection. A tag carried by only some of the selected assets is shown with a dashed outline: click
it to add it to all of them.

## Finding assets

Click the search field, press <kbd>Ctrl</kbd>+<kbd>F</kbd>, or simply start typing. Pigoune
searches names, tags, notes, sources, licenses and authors, ignoring case and accents, within
the entry selected in the sidebar. Select **All** to search the whole library.

The **Filters** button keeps only some formats, or only your favorites.

To find out where an asset is stored, look at **Stored In** at the bottom of the details
panel. Clicking a collection there opens it in the sidebar and highlights the asset. Clicking
a tag in the details panel works the same way.

## Looking at assets

- Change the thumbnail size with the slider above the grid, or with
  <kbd>Ctrl</kbd>+<kbd>+</kbd> and <kbd>Ctrl</kbd>+<kbd>-</kbd>. The slider sets the smallest
  size: thumbnails grow slightly so that each row fills the whole width of the window.
- Sort the grid by date added, name, type, dimensions or size with the sort button.
- Animated GIFs play when you hover over them.
- Press <kbd>Space</kbd> or double-click an asset to open the **detailed preview**. Zoom with
  the scroll wheel or with <kbd>+</kbd> and <kbd>-</kbd>, use <kbd>0</kbd> to fit the window
  and <kbd>1</kbd> for the actual size, and choose a background color from the top bar. Press
  <kbd>Space</kbd> or <kbd>Esc</kbd> to go back.
- Move to the previous or next asset with the arrow keys, with the arrow buttons that appear
  when you move the mouse, or with a two-finger swipe. <kbd>Home</kbd> and <kbd>End</kbd> jump
  to the first and last asset.
- Press <kbd>F11</kbd>, or choose **Full Screen** in the zoom menu, to fill the screen; the top
  bar comes back when you move the mouse, and <kbd>Esc</kbd> leaves full screen.
- For an ICO file holding several sizes, buttons below the image show each size as it was drawn
  (16, 32, 48, 256…).
- In the preview, drag the image to examine any part of it, even a corner brought to the
  middle of the screen. A dashed outline shows the real edges of the image, transparent
  margins included; turn it off with **Show Image Bounds** in the zoom menu.
- From 800% on, a light grid separates the pixels of pictures, which helps to check icons and
  pixel art; turn it off with **Show Pixel Grid** in the zoom menu.
- The star next to the back button adds the shown asset to your favorites, and a right-click on
  the image offers **Copy**, **Open With…** and **Export To…** for it.

## Reusing assets

- **Drag** assets from the grid into any application (a text editor, a design tool, a web
  page…). Pigoune hands over a copy named after the asset.
- **Copy** them with <kbd>Ctrl</kbd>+<kbd>C</kbd> and paste them elsewhere.
- **Export** them to a folder with **Export To…** in the right-click menu. Existing files are
  never overwritten.
- **Open** an asset in another application, such as an image editor, with **Open With…** in
  the right-click menu. The application receives a copy: your library stays untouched, so
  save your changes under a new name and import them if you want to keep them.

## Trash and undo

Press <kbd>Delete</kbd>, or choose **Move to Trash** in the right-click menu, to send assets
to the trash. Nothing is lost until you empty it: open **Trash** in the sidebar to restore
assets or to empty it for good.

Most changes can be undone with <kbd>Ctrl</kbd>+<kbd>Z</kbd> or with the **Undo** button of
the message that appears after an action: trashing, moving, tags, favorites, renaming, notes
and credits. Imports cannot be undone, and emptying the trash clears the undo history.

## Your library

A library is a regular folder whose name ends with `.pigoune`. It contains:

- `files/`, a copy of every asset, with its original file name;
- `library.db`, the database holding collections, tags and all other information;
- `cache/`, thumbnails that Pigoune can rebuild at any time.

Because everything is inside this folder, a library is **portable**: copy it to an external
drive or to another computer and open it there with Pigoune. To back it up, copy the whole
folder while Pigoune is closed.

A library can only be open in one Pigoune window at a time. If you keep a library in a synced
folder (Nextcloud, Syncthing…), close Pigoune on one computer before opening it on another.

## Preferences

Open **Preferences** from the main menu, or press <kbd>Ctrl</kbd>+<kbd>,</kbd>:

- **Show Resource Names** under each thumbnail of the grid. When names are hidden, hover over a
  thumbnail to see its name.
- **Show Resource Counts** next to each entry of the sidebar.
- **Reopen the Last Entry**: a library opens on the entry you used last instead of **All**.
- **Confirm Before Emptying the Trash**.
- **Empty the Trash Automatically**: assets are deleted for good after 30 days in the trash.

## Keyboard shortcuts

Press <kbd>Ctrl</kbd>+<kbd>?</kbd> to see every shortcut in Pigoune.

| Action | Shortcut |
|---|---|
| New library | <kbd>Ctrl</kbd>+<kbd>N</kbd> |
| Open a library | <kbd>Ctrl</kbd>+<kbd>O</kbd> |
| Import files | <kbd>Ctrl</kbd>+<kbd>I</kbd> |
| Search | <kbd>Ctrl</kbd>+<kbd>F</kbd> |
| Detailed preview | <kbd>Space</kbd> |
| Select all / deselect all | <kbd>Ctrl</kbd>+<kbd>A</kbd> / <kbd>Esc</kbd> |
| Copy | <kbd>Ctrl</kbd>+<kbd>C</kbd> |
| Rename | <kbd>F2</kbd> |
| Add to or remove from favorites | <kbd>Ctrl</kbd>+<kbd>D</kbd> |
| Move to trash | <kbd>Delete</kbd> |
| Undo | <kbd>Ctrl</kbd>+<kbd>Z</kbd> |
| Larger / smaller thumbnails | <kbd>Ctrl</kbd>+<kbd>+</kbd> / <kbd>Ctrl</kbd>+<kbd>-</kbd> |
| Context menu | <kbd>Menu</kbd> or <kbd>Shift</kbd>+<kbd>F10</kbd> |
| Help | <kbd>F1</kbd> |
| Preferences | <kbd>Ctrl</kbd>+<kbd>,</kbd> |
| Quit | <kbd>Ctrl</kbd>+<kbd>Q</kbd> |

## Questions and answers

**Can I delete my original files after importing them?**
Yes. Pigoune works on its own copies, stored inside the library.

**Does Pigoune send anything over the internet?**
No. Pigoune works fully offline, with no account and no telemetry.

**Why does Pigoune say a file is unreadable?**
The file is damaged, or is not really in the format its name suggests. Pigoune checks the
content of each image, not just its extension.

**I imported the same file twice. Where is the second copy?**
There is none: Pigoune recognizes duplicates and adds the existing asset to the target
collection instead.

**How do I report a bug or suggest an idea?**
Open an issue on [GitHub](https://github.com/Gor3pig/Pigoune/issues).
