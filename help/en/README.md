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
**New Library…**, give it a name and pick where to save it. You can create as many
libraries as you like, for example one per project or per client.

To open an existing library, choose **Open Library…**. Pigoune reopens the last library you
used every time it starts. While a library is open, the main menu offers **New Library…** and
**Open Library…** to switch to another one, and **Recent Libraries** lists the last libraries
you opened, five by default. The welcome page shows them too, so you can reopen one in a
click. On the welcome page, remove one from the list with its cross, or choose **Clear the
List** there or in the menu: only the list changes, the libraries themselves are kept.

**Close Library** in the main menu brings you back to the welcome page. The next time Pigoune
starts, it opens on the welcome page too.

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
sub-collection, rename it (<kbd>F2</kbd>), customize it or delete it.

- **Customize** a collection to give it its own icon and color in the sidebar: right-click it,
  choose **Customize…**, pick a color and an icon, then **Save**. **Default** brings back the
  gray folder.
- **Move** assets into a collection by dragging them onto it from the grid.
- **Add** them to a collection while keeping them where they are by holding <kbd>Ctrl</kbd>
  while dragging, or with **Add to a Collection…** in the right-click menu. An asset can belong
  to several collections.
- **Remove** them from the collection you are looking at with **Remove from the Collection** in
  the right-click menu. They stay in the library and in their other collections.
- **Reorder** collections by dragging them in the sidebar, or sort them by name or creation
  date with the **⋯** button next to **Collections**.
- **Fold** a section of the sidebar (Collections, Smart Collections or Tags) with a click on
  its title, or with <kbd>Enter</kbd> when the title has the keyboard focus. Pigoune remembers
  which sections are folded.
- **Unclassified** lists the assets that belong to no collection.

When you delete a collection, its sub-collections are deleted too, and the assets that only
belonged to them go to the trash. Assets that also belong to another collection stay there.

### Tags

A tag is a single word of up to 20 characters: it describes assets in one keyword. To join
several words, use a hyphen (*flat-design*). A space or a comma ends the tag you are typing, so
you can enter several at once. Add tags from the details panel with the
**+** button next to the tags, with **Add a Tag…** in the right-click menu, or drag assets onto
a tag in the sidebar. The sidebar shows tags as pills with their number of assets: click one
to see all its assets, or right-click it to rename or delete it. Beyond twelve tags, **+ N
others** shows the rest. Renaming a tag to the name of another one merges them.

### Favorites

Mark an asset as a favorite with the star next to its name in the details panel, or press
<kbd>Ctrl</kbd>+<kbd>D</kbd>. Favorites have their own entry in the sidebar.

### Names, notes and credits

Click the name of an asset in the details panel (or press <kbd>F2</kbd>) to rename it. The
**Note and Credits** group lets you write a note and record the source, the license and the
author of each asset, which is handy when you reuse it later. The **File** group below shows
when the asset was added and the name of the original file. Each group folds and unfolds with
a click on its title, which also shows a short summary, and Pigoune remembers which ones are
open. Under the name, **Open With…**, **Copy** and **Export…** act on the asset directly.

### Several assets at once

Select several assets with <kbd>Ctrl</kbd>+click, <kbd>Shift</kbd>+click, by drawing a
rectangle from an empty area of the grid, or with <kbd>Ctrl</kbd>+<kbd>A</kbd>. The details
panel then shows a summary and lets you change favorites and tags for all of them, while
dragging and the right-click menu work on the whole selection. A tag carried by only some of the selected assets is shown with a dashed outline: click
it to add it to all of them.

## Finding assets

Click the search field, press <kbd>Ctrl</kbd>+<kbd>F</kbd>, or simply start typing. Pigoune
searches names, tags, notes, sources, licenses and authors, ignoring case, accents and ligatures (*coeur* finds
*cœur*), within the entry selected in the sidebar. Select **All** to search the whole library.

- **Space means and.** `logo goat` finds the assets that contain *logo* **and** *goat*, even in
  different places (for example *logo* in the name and *goat* in a tag).
- **Comma means or.** `logo, goat` finds the ones that contain *logo* **or** *goat*.
- **Commas cut the search into groups**: an asset is found as soon as it contains all the words
  of one group. `red logo, goat` finds what contains both *red* and *logo*, or else *goat*.
- A word can be just part of a word: `cat` also finds *category*.
- With two words or more, they appear as pills under the search field, joined by **and** or
  **or**. Clicking an **or** joins the two neighboring groups; clicking an **and** cuts the
  group in two at that place. In `red logo, goat`, clicking the **or** gives `red logo goat`
  (all three words are required); clicking the **and** gives `red, logo, goat` (any one is
  enough). The cross of a pill removes its word. The same pills appear under **Words to Find**
  in the smart collection window.
- A search uses at most 8 words; the next ones are ignored, and the pills say so.

The search does not look at the format: to keep only SVG files, for example, use the
**Filters** button, which can also keep only your favorites.

**Filters** also offers thirteen colors. Pigoune notes the main colors of each asset, up to
three that each cover at least 15% of the visible picture, transparent areas left aside. Pick
red and blue to see the assets that are mainly red or blue. The last, rainbow swatch opens the
GNOME color chooser to pick any color, for example the exact color of a brand: Pigoune then
keeps the assets with a main color close to it. Click it again to remove it. Colors add up
with types and favorites: red and SVG show the red SVG files. When a library is opened with Pigoune 2.0 for
the first time, its assets are analyzed in the background while you keep working; an asset
not analyzed yet matches no color.

**Shape** keeps the assets in **Landscape**, **Portrait** or **Square** format; pick two
shapes to see both. An image counts as square when its sides differ by 5% at most.
**Fits My Screen** keeps the images at least as large as the screen Pigoune is shown on, in
real pixels and in both directions, for example 1920 × 1080 or larger: they fill the screen
without being enlarged, so without blur. SVG files are always left aside.
Together, **Landscape** and **Fits My Screen** find good wallpapers.

To find out where an asset is stored, look at **Collections** in the **Organization** group of
the details panel. Clicking a collection there opens it in the sidebar and highlights the
asset. Clicking a tag works the same way. **Detected Colors** shows the main colors Pigoune
found in the asset; to keep only the assets of a color, use the **Filters** button.

### Smart collections

A smart collection is a saved search that keeps itself up to date: it always shows the assets
that match its criteria, including the ones you import later. For example, *all my favorite
SVG files* or *every landscape image that fits my screen*. It always searches the whole
library, the trash left aside.

- **Create** one with the **+** button next to **Smart Collections** in the sidebar. The window
  is filled in with the search and the filters in use, and with **Favorites Only** when
  **Favorites** is open. Give it a name, then choose words to find, types, shapes, colors, **Fits My Screen**
  or **Favorites Only**: at least one criterion is needed. **Fits My Screen** follows the screen
  Pigoune is shown on when you open the smart collection.
- **Words to Find** works exactly like the search field: spaces and commas, and pills whose
  **and** and **or** cut or join the groups.
- **Open** it from the sidebar to see its assets. You can still search inside it.
- **Change** its name or criteria with **Edit…** in its right-click menu, or with
  <kbd>F2</kbd>. **Delete…** only deletes the saved search: the assets stay in the library.
- **Reorder** smart collections by dragging them in the sidebar, or sort them by name or
  creation date with the **⋯** button next to their title.

Assets cannot be dropped onto a smart collection, since its content follows its criteria.

## Looking at assets

- Change the thumbnail size with the slider above the grid, or with
  <kbd>Ctrl</kbd>+<kbd>+</kbd> and <kbd>Ctrl</kbd>+<kbd>-</kbd>. The slider sets the smallest
  size: thumbnails grow slightly so that each row fills the whole width of the window.
- Sort the grid by date added, name, type, dimensions or size with the sort button.
- Animated GIF, PNG and WebP images play when you hover over them.
- Press <kbd>Space</kbd> or double-click an asset to open the **detailed preview**. Zoom with
  the scroll wheel or with <kbd>+</kbd> and <kbd>-</kbd>, use <kbd>0</kbd> to fit the window
  and <kbd>1</kbd> for the actual size, and choose a background color with the **Background**
  button of the top bar. The **Open With…**, **Copy** and **Export To…** buttons next to it act
  on the asset shown. The **Show Details** button opens the details panel next to the asset, to
  tag it, file it or add a note without leaving the preview; Pigoune remembers whether it is
  open. Press <kbd>Space</kbd> or <kbd>Esc</kbd> to go back.
- Move to the previous or next asset with the arrow keys, with the arrow buttons that appear
  when you move the mouse, or with a two-finger swipe. <kbd>Home</kbd> and <kbd>End</kbd> jump
  to the first and last asset.
- Press <kbd>F11</kbd>, or choose **Full Screen** in the zoom menu, to fill the screen; the top
  bar comes back when you move the mouse, and <kbd>Esc</kbd> leaves full screen.
- For an ICO file holding several sizes, buttons below the image show each size as it was drawn
  (16, 32, 48, 256…).
- For an animation, a bar below the image pauses and resumes it and steps through it frame by
  frame, and shows which frame is displayed. From the keyboard, <kbd>K</kbd> pauses or resumes,
  <kbd>,</kbd> and <kbd>.</kbd> show the previous and next frame.
- In the preview, drag the image to examine any part of it, even a corner brought to the
  middle of the screen. A dashed outline shows the real edges of the image, transparent
  margins included; turn it off with **Show Image Bounds** in the zoom menu.
- From 800% on, a light grid separates the pixels of pictures, which helps to check icons and
  pixel art; turn it off with **Show Pixel Grid** in the zoom menu.
- A strip of thumbnails at the bottom of the preview shows the neighboring assets; click one to
  show it. It is hidden in full screen and in narrow windows; turn it off with **Show Thumbnail
  Strip** in the zoom menu.
- The star next to the back button adds the shown asset to your favorites, and a right-click on
  the image offers **Open With…**, **Frame and Set as Wallpaper…**, **Copy**, **Export To…**, **Export
  As…** and **Add to Favorites** for it.

## Reusing assets

- **Drag** assets from the grid into any application (a text editor, a design tool, a web
  page…). Pigoune hands over a copy named after the asset.
- **Copy** them with <kbd>Ctrl</kbd>+<kbd>C</kbd> and paste them elsewhere.
- **Export** them to a folder with **Export To…** in the right-click menu. Existing files are
  never overwritten.
- **Convert** them with **Export As…** in the right-click menu: choose PNG, JPEG, WebP, AVIF or
  ICO, then a folder. JPEG and AVIF offer a quality setting. JPEG has no transparency, so a
  background color fills the transparent areas; the other formats keep transparency unless you
  turn off **Keep Transparency** to use a background color too. Choose the width and height, in
  pixels or as a percentage; 100% keeps the original size. With the padlock closed, proportions
  are kept: changing one side updates the other, and several images of different shapes each fit
  inside the given width and height. Open it to stretch an image freely. To fit several images
  to your screen at once, turn on **Size of My Screen**: each image takes exactly the size of
  the screen Pigoune is shown on, for example 1920 × 1080. **Fill the Screen** covers the whole
  screen and cuts the edges that overflow, around the center; **Whole Image** keeps the whole
  image and fills the bands with the background color, or leaves them transparent when
  transparency is kept. SVG images stay sharp
  at any size; an enlarged pixel image becomes blurry. A resized file carries its size in its
  name, such as `logo-512x384.png`. From the preview, an animation paused on a frame exports
  that frame, named for instance `spinner-frame-3.png`. For ICO, tick the sizes to include (16,
  32, 48 and 256 by default): they all go into a single icon file. Pigoune remembers your
  format, quality, background and unit for the next export, while the size always starts from
  the original. The original stays untouched in the library, and existing files are never
  overwritten.
- **Open** an asset in another application, such as an image editor, with **Open With…** in
  the right-click menu. The application receives a copy: your library stays untouched, so
  save your changes under a new name and import them if you want to keep them.
- **Set an image as your wallpaper** with **Frame and Set as Wallpaper…** in the right-click
  menu of the grid or of the preview. A large window shows a virtual screen at the real
  resolution of the screen Pigoune is shown on, for example 1920 × 1080, with the image framed
  the way GNOME would. Drag the image to move it, zoom with the mouse wheel, the slider or
  <kbd>+</kbd> and <kbd>-</kbd>, and adjust it with the arrow keys (<kbd>Shift</kbd> moves
  further). **Fill** comes back to the framing of GNOME, **Whole** shows all of the image and
  **100%** shows one pixel of the image per pixel of the screen. **Mirror** flips the image from
  left to right in the wallpaper only, **Thirds** shows the rule of thirds grid to place the
  subject, and **Snap**, on at first, makes the dragged image catch the center and the edges of
  the screen, a blue line showing where. What goes past the screen stays visible, faded. When the image is enlarged past 100%, a notice over the screen warns that it
  will look blurry. The full screen button, or <kbd>F11</kbd>, shows the virtual screen at its
  real size, as a simulation that a short notice recalls; <kbd>Esc</kbd> comes back.
- **Darken**, under **Image**, darkens the whole wallpaper, up to 60%, so that icons and bars
  stay readable; like **Mirror**, it changes the wallpaper only, never the asset. With several
  screens, **Prepare For** chooses the one the wallpaper is made for: GNOME shows the same
  wallpaper on every screen and adapts it to the others.
- **Desktop Simulation** draws an imitation of the bars of your desktop over the virtual
  screen: GNOME, KDE Plasma, Cinnamon, Xfce, MATE, COSMIC or Budgie, the one in use being
  chosen at first. Turn it off to see the image alone.
- Under **Empty Space**, choose what fills the screen around a smaller image: a **Color**,
  black at first, a **Gradient**, **Blur**, the same image enlarged and
  blurred behind it, or **Mosaic**, the image repeated over the whole screen, handy for
  patterns. Colors are picked from swatches showing the main colors of the image, black and
  white, and the rainbow swatch opens the GNOME color chooser. A gradient starts from the two main
  colors of the image, from top to bottom; **Angle** turns it (0° goes up, 90° to the right,
  180° down), the **Start Color** and **End Color** buttons open the GNOME color chooser, and
  **Swap Colors** exchanges them.
- **Set as Wallpaper** makes an image exactly the size of the screen, so the desktop shows
  exactly what you framed; a small window shows the preparation, then GNOME asks you to confirm.
  Tick **Also Add the Edited Version to the Library** to keep that image as a new asset.

## Trash and undo

Press <kbd>Delete</kbd>, or choose **Move to Trash** in the right-click menu, to send assets
to the trash. Nothing is lost until you empty it: open **Trash** at the bottom of the sidebar
to restore assets, with **Restore** in the right-click menu or in the details panel, or to
empty it for good.

Most changes can be undone with <kbd>Ctrl</kbd>+<kbd>Z</kbd> or with the **Undo** button of
the message that appears after an action: trashing, moving, tags, favorites, renaming, notes
and credits, but also creating, renaming, customizing, moving or deleting a collection, and
creating, editing, reordering or deleting a smart collection. Imports cannot be undone, and emptying the
trash clears the undo history. Creating a collection is only undone while it is still empty:
once assets are imported into it, use **Delete…** instead.

## Your library

A library is a regular folder whose name ends with `.pigoune`. It contains:

- `files/`, a copy of every asset, with its original file name, sorted into small subfolders
  so that even very large libraries stay easy to handle;
- `library.db`, the database holding collections, tags and all other information;
- `cache/`, thumbnails that Pigoune can rebuild at any time.

Because everything is inside this folder, a library is **portable**: copy it to an external
drive or to another computer and open it there with Pigoune. To back it up, copy the whole
folder while Pigoune is closed.

A library can only be open in one Pigoune window at a time. If you keep a library in a synced
folder (Nextcloud, Syncthing…), close Pigoune on one computer before opening it on another.

Choose **Library Information** in the main menu to see what the open library holds, in three
tabs:

- **Overview** shows the name and location of the library, the number of assets, collections,
  tags and favorites, and the **Records**: the heaviest, the largest, the newest and the oldest
  asset; click one to see it in the grid.
- **Content** shows how the space is shared between formats in a ring chart, by size or by
  number of assets, the number of animated images, SVG images and assets in the trash, and the
  detected colors with the number of assets where each one is a main color.
- **Storage** shows in a single bar what the library takes on its disk, split between the
  assets, the thumbnails, the database and the trash, next to the other files and the free
  space. Below are where the library is stored, with buttons to copy its location or open its
  folder, whether that disk is removable, when the library was created and which versions of
  Pigoune can open it.

## Preferences

Open **Preferences** from the main menu, or press <kbd>Ctrl</kbd>+<kbd>,</kbd>. The settings are
split into three pages; the magnifier at the top finds a setting by its name.

**General**

- **Interface Language**: Pigoune follows the language of your system; choose another of the
  languages Pigoune is translated into to use it instead. The change applies the next time
  Pigoune starts, and **Restart** in the message that appears does it at once. A few windows
  provided by GNOME, such as the one to choose a folder, may stay in the language of the
  system.
- **Reopen the Last Library**: at startup, Pigoune opens the library you left open. Turn it off
  to start from the welcome page and choose a library each time.
- **Recent Libraries**: how many recent libraries the main menu and the welcome page offer,
  from 0 to 8. Choose 0 to turn the list off.
- **Reopen the Last Entry**: a library opens on the entry you used last instead of **All**.
- **Show What’s New After an Update**: the first time a new version starts, a short window lists
  its main news. Turn it off if you prefer not to see it; the About window keeps the news of
  the current version.
- **Confirm Before Emptying the Trash**.
- **Empty the Trash Automatically**: assets are deleted for good after 30 days in the trash.
- **Thumbnails**, under **Storage**, shows the space taken by the thumbnails of the open
  library. **Clear** frees it; thumbnails are made again when they are needed.

**Display**

- **Show Resource Names** under each thumbnail of the grid. When names are hidden, hover over a
  thumbnail to see its name.
- **Show Formats**: a badge shows the format of each asset (SVG, PNG…) on its thumbnail.
- **Thumbnail Background**: white, gray, black or a checkerboard behind the thumbnails, to see
  white or black images and transparent areas.
- **Play Animations on Hover**: turn it off if moving thumbnails distract you; animations still
  play in the details panel and the preview.
- **Show Resource Counts** next to each entry of the sidebar.
- **Show Tags**: turn it off to hide the tags section of the sidebar. Tags still appear in the
  details panel, and searches still find them.
- **Show Smart Collections**: turn it off to hide the smart collections section of the
  sidebar. Your smart collections are kept and come back when you turn it on again.

**Behavior**

- **Open Resources on Double-Click**: a double-click opens the asset in its default
  application instead of the preview. <kbd>Space</kbd> still opens the preview.
- **Search the Whole Library**: searches and filters look everywhere instead of only in the
  entry selected in the sidebar, except in the trash and in smart collections.

Pigoune follows the style and the accent color you choose in GNOME **Settings**, under
**Appearance**: light or dark style, and the color of selections, switches and highlights.

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
| Rename or edit the entry selected in the sidebar | <kbd>F2</kbd> |
| Delete the collection selected in the sidebar | <kbd>Delete</kbd> |
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
No. Pigoune works fully offline, with no account and no telemetry. Checking for updates is done
by Flatpak, which only asks the Pigoune repository whether a new version exists.

**How do I update Pigoune?**
Software takes care of it: it offers each new version, and installs it by itself when automatic
updates are turned on. `flatpak update` installs it too. If Pigoune stays open, a banner at the
top of the window also says so within half an hour: click **Update**, wait for the
installation, then click **Restart**.

**Why does Pigoune say a file is unreadable?**
The file is damaged, or is not really in the format its name suggests. Pigoune checks the
content of each image, not just its extension.

**I imported the same file twice. Where is the second copy?**
There is none: Pigoune recognizes duplicates and adds the existing asset to the target
collection instead.

**How do I report a bug or suggest an idea?**
Open an issue on [GitHub](https://github.com/Gor3pig/Pigoune/issues).
