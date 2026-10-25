# Pigoune user guide

*[Version française](../fr/README.md)*

Pigoune keeps all your graphic assets (icons, logos, illustrations) in one tidy library. This guide shows you how to use it, from your first import to the advanced settings.

<img src="../../data/screenshots/01.png" alt="The main window of Pigoune: the sidebar with collections and tags, a grid of icons and the details of the selected asset" width="800">

## Contents

**Discover**

- [Getting started](#getting-started): create a library and bring assets in
- [Organizing](#organizing): collections, tags and favorites
- [Finding assets](#finding-assets): search, filters and smart collections
- [Looking at assets](#looking-at-assets): the grid and the detailed preview

**Use**

- [Reusing assets](#reusing-assets): drag, export, convert, wallpapers
- [Capturing and annotating](#capturing-and-annotating): screenshots and annotations
- [Trash and undo](#trash-and-undo): fix any mistake

**Go further**

- [Your library](#your-library): storage, health, duplicates, export
- [Preferences](#preferences): the settings
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Questions and answers](#questions-and-answers)

## Getting started

> [!NOTE]
> A **library** is a folder where Pigoune keeps a copy of your assets. Create one, then import your files into it.

### Create a library

You can create as many libraries as you like, for example one per project or per client.

1. On the welcome screen, choose **New Library…**.
2. Give the library a name.
3. Pick where to save it.

To open an existing library, choose **Open Library…**.

### Switch between libraries

Pigoune reopens the last library you used every time it starts.

While a library is open, the main menu lets you:

- choose **New Library…** or **Open Library…** to switch to another one;
- choose **Close Library** to go back to the welcome page, which Pigoune also shows at the next start.

#### Recent libraries

**Recent Libraries** in the main menu lists the last libraries you opened, five by default. The welcome page shows them too, so you can reopen one in a click.

To clean the list, remove a library with its cross on the welcome page, or choose **Clear the List** there or in the menu. Only the list changes: the libraries themselves are kept.

### Import assets

There are four ways to bring assets in.

| Way | How |
|---|---|
| **Import menu** | Click **+** at the top of the sidebar, or press <kbd>Ctrl</kbd>+<kbd>I</kbd>, then choose **Import Files…** or **Import Folder…**. |
| **Drag and drop** | Drag files or folders from Files onto the window, or onto **All**, **Unclassified** or a collection in the sidebar. |
| **Copy and paste** | Copy files in Files, or an image (a screenshot, an image from a web page), then press <kbd>Ctrl</kbd>+<kbd>V</kbd>. |
| **From a web page** | Drag an image from a web page onto the window, or onto a collection of the sidebar. |

A pasted image is named **Pasted image** followed by the date and time; you can rename it afterwards.

<details>
<summary>What the browser hands over</summary>

Pigoune keeps what the browser gives it: the original file with Chrome or Brave, a PNG copy of the image with Firefox. It names the asset after the file name the browser suggests. If the drop holds no image, a message says so.

</details>

#### What Pigoune accepts

| Topic | Rule |
|---|---|
| **Formats** | SVG, PNG, JPEG, WebP, AVIF, HEIC, JPEG XL, GIF, TIFF, BMP and ICO. |
| **Checks** | Every image is checked before it is imported. A damaged file, or one that cannot be shown, is listed and refused. |
| **Duplicates** | A file already in the library is never copied twice. If it is in the trash, importing it takes it out of the trash. |
| **Folders** | When you import a folder, its subfolders become collections. |

HEIC photos are shown when the system can decode them, which the Flatpak package does.

When you import a file that is already there into a collection, the existing asset is added to that collection and stays in the others. If it was in the trash, it is restored and then belongs to that collection only.

> [!TIP]
> Your original files are never modified or moved. You can delete them once they are imported.

#### Where the files go

Files are imported into the view you are looking at.

| View | What happens |
|---|---|
| **All** and **Unclassified** | The files are kept without a collection. |
| A selected collection | The collection receives them. |
| Favorites, tags, smart collections, Trash | They only show assets, so importing is turned off. |

*See also: [Organizing](#organizing), [Your library](#your-library).* · [Back to contents](#contents)

## Organizing

Collections, tags and favorites keep your library tidy. Use the ones that match the way you think.

### Collections

Collections work like folders, and can contain other collections.

#### Create and manage

Create a collection with the **+** button next to **Collections** in the sidebar.

Then right-click it to create a sub-collection, rename it (<kbd>F2</kbd>), customize it, move it with **Move to…** (into another collection or to the top level) or delete it. The tags menu works the same way.

| To… | Do this |
|---|---|
| **Customize** | Right-click, choose **Customize…**, pick a color and an icon, then **Save**. **Default** brings back the gray folder. |
| **Reorder** | Drag collections in the sidebar, or sort them by name or creation date with the **⋯** button next to **Collections**. |
| **Fold** | Click the title of a sidebar section, or press <kbd>Enter</kbd> when it has the keyboard focus. Pigoune remembers which sections are folded. |

**Unclassified** lists the assets that belong to no collection.

#### Put assets in collections

| To… | Do this |
|---|---|
| **Move** assets into a collection | Drag them onto it from the grid, or, inside a collection, use **Move to a Collection…** in the right-click menu. |
| **Add** them and keep them where they are | Hold <kbd>Ctrl</kbd> while dragging, or use **Add to a Collection…** in the right-click menu. |
| **Remove** them from the collection you are looking at | Use **Remove from the Collection** in the right-click menu. |

An asset can belong to several collections. Removing it from one keeps it in the library and in its other collections.

While you drag, the pointer shows what you are doing: a plain arrow for a move, a **+** for an addition. It changes as soon as you press or release <kbd>Ctrl</kbd>.

> [!WARNING]
> When you delete a collection, its sub-collections are deleted too, and the assets that only belonged to them go to the trash. Assets that also belong to another collection stay there.

### Tags

A tag is a keyword that describes an asset. Tags can be nested, like *Subject › Animals › Goat*.

#### Write and add tags

A tag is a single word of up to 20 characters. To join several words, use a hyphen (*flat-design*).

A space or a comma ends the tag you are typing, so you can enter several at once.

| To add tags… | Do this |
|---|---|
| **From the details panel** | Click the **+** button next to the tags. |
| **From the menu** | Choose **Add a Tag…** in the right-click menu. |
| **By dragging** | Drag assets onto a tag in the sidebar. |

Keep dragged assets over a tag that has sub-tags for a second: its sub-tags open, so you can drop the assets on one. The sidebar goes back to where it was afterwards.

#### Browse tags in the sidebar

The sidebar shows tags as pills with their number of assets.

- Click a pill to see all its assets.
- Right-click it to rename or delete it.
- Beyond twelve tags, **+ N others** shows the rest.

Renaming a tag to the name of another one merges them.

#### Nested tags

Type a path with a slash, for example `animals/goat`, to create or reuse each level. Each level is a single word of up to 20 characters.

| What | How it works |
|---|---|
| **Suggestions** | Typing `animals/` suggests its sub-tags, and every suggestion shows its parents. |
| **Plain name** | A plain name followed by <kbd>Enter</kbd> reuses that tag wherever it is. If several tags share it, pick one in the list. |
| **Same name** | The same name can exist in two different branches, but not twice at the same level. |
| **Search** | Searching a parent's name also finds the assets of its sub-tags. |

<img src="../../data/screenshots/10.png" alt="Tags nested level by level in the sidebar, with the path Tags, subject, animals above the pills" width="720">

#### Move around nested tags

The Tags section of the sidebar always mirrors what the grid shows.

| Gesture | Result |
|---|---|
| Click the name of a pill | The grid shows that tag with the assets of its sub-tags, and its sub-tags open in the sidebar. |
| Click the path above the pills | You climb back up. **Tags** returns to the first level and shows the whole library again. |
| Choose anything else in the sidebar | The Tags section goes back to the first level. |

The search label and the messages name a tag with its full path, for example *subject › animals*. If some of the assets you had selected are not in the new view, a short message says how many.

#### Reorganize tags

Right-click a tag for **New Sub-tag…**, **Move to…**, **Merge into…** and **Delete…**. You can also drag a pill onto another tag, onto a level of the path, or onto **Tags** to move it.

Every change can be undone. When you delete a tag that has sub-tags, you choose to delete them too or to move them up one level.

<details>
<summary>If you delete the tag you are looking at</summary>

Pigoune shows its parent instead, and undoing brings you back to the tag.

</details>

> [!NOTE]
> Your existing tags become first-level tags and nothing is lost.

### Favorites

Favorites have their own entry in the sidebar. To add an asset to them:

- click the star next to its name in the details panel;
- press <kbd>Ctrl</kbd>+<kbd>D</kbd>;
- or drag assets onto **Favorites** in the sidebar.

### Names, notes and credits

In the details panel:

| What | How |
|---|---|
| **Rename** | Click the name of the asset, or press <kbd>F2</kbd>. |
| **Note and Credits** | Write a note and record the source, the license and the author, which is handy when you reuse the asset later. |
| **File** | Shows when the asset was added and the name of the original file. |
| **Quick actions** | Under the name, **Open With…**, **Copy** and **Export…** act on the asset directly. |

Each group folds and unfolds with a click on its title. The title also shows a short summary. Pigoune remembers which ones are open.

### Several assets at once

Select several assets with <kbd>Ctrl</kbd>+click, <kbd>Shift</kbd>+click, a rectangle drawn from an empty area of the grid, or <kbd>Ctrl</kbd>+<kbd>A</kbd>.

The details panel then shows a summary and lets you change favorites and tags for all of them. Dragging and the right-click menu work on the whole selection.

> [!TIP]
> A tag carried by only some of the selected assets has a dashed outline and its count (such as 2/5). Click its name to open it, or the small **+** next to it to add it to all of them.

*See also: [Finding assets](#finding-assets), [Trash and undo](#trash-and-undo).* · [Back to contents](#contents)

## Finding assets

Search by words, narrow the grid with filters, and save a search that keeps itself up to date.

### Search

Click the search field, press <kbd>Ctrl</kbd>+<kbd>F</kbd>, or simply start typing.

Pigoune searches names, tags, notes, sources, licenses and authors. It ignores case, accents and ligatures (*coeur* finds *cœur*), and a word can match just part of a longer word: `cat` also finds *category*.

The search does not look at the format. To keep only SVG files, for example, use the **Filters** button.

#### Where it looks

A small label at the start of the search field says where Pigoune looks: **Everywhere**, or the name of the entry selected in the sidebar. Click it to choose **In the Whole Library** or **In** followed by the name of the entry.

- When you choose an entry in the sidebar while a search is looking everywhere, the grid shows that entry with your search applied inside it. Typing again widens the search as before.
- In **All**, the trash and smart collections, the label stays without a menu, because the search always looks there.

In a narrow window, the field gives way to a magnifier button at the top right. Click it, press <kbd>Ctrl</kbd>+<kbd>F</kbd> or start typing: a full-width search bar opens under the title bar, with the same label and the **Filters** button. <kbd>Esc</kbd> closes it and clears the search.

#### Several words

| You type | Pigoune finds |
|---|---|
| `logo goat` | Assets containing *logo* **and** *goat*, even in different places |
| `logo, goat` | Assets containing *logo* **or** *goat* |
| `red logo, goat` | Assets containing both *red* and *logo*, or else *goat* |

A **space** means *and*, a **comma** means *or*. Commas cut the search into groups: an asset is found as soon as it contains all the words of one group, wherever those words are. For example, *logo* can be in the name and *goat* in a tag.

#### Word pills

With two words or more, the words appear as pills under the search field, joined by **and** or **or**.

- Click an **or** to join the two neighboring groups.
- Click an **and** to cut the group in two at that place.
- Click the cross of a pill to remove its word.

In `red logo, goat`, clicking the **or** gives `red logo goat` (all three words are required). Clicking the **and** gives `red, logo, goat` (any one is enough).

The same pills appear under **Words to Find** in the smart collection window. A search uses at most 8 words: the next ones are ignored, and the pills say so.

### Filters

The **Filters** button narrows the grid by type, favorites, color and orientation. Filters add up with your search and with each other.

<img src="../../data/screenshots/02.png" alt="The Filters menu open over the grid, with the types, the orientations and a row of colors" width="720">

#### Types and favorites

Tick one or several formats to keep only those, for example SVG. **Favorites Only** keeps your favorites.

#### Colors

Pigoune notes the main colors of each asset, up to three that each cover at least 15% of the visible image, transparent areas left aside. **Filters** offers thirteen colors: pick red and blue to see the assets that are mainly red or blue.

The last, rainbow swatch opens the GNOME color chooser to pick any color, for example the exact color of a brand. Pigoune then keeps the assets with a main color close to it. Click the swatch again to remove it.

Colors add up with types and favorites: red and SVG show the red SVG files.

> [!NOTE]
> When an older library is opened for the first time, its assets are analyzed in the background while you keep working. An asset not analyzed yet matches no color.

#### Orientation and screen size

**Orientation** keeps the assets in **Landscape**, **Portrait** or **Square** format. Pick two orientations to see both. An image counts as square when its sides differ by 5% at most.

**Fits My Screen** keeps the images at least as large as the screen Pigoune is shown on, in real pixels, in width and in height, for example 1920 × 1080 or larger. They fill the screen without being enlarged, so without blur. SVG files are always left aside.

> [!TIP]
> **Landscape** and **Fits My Screen** together find good wallpapers.

### Find where an asset is stored

Look at **Collections** in the **Organization** group of the details panel. Clicking a collection there opens it in the sidebar and highlights the asset. Clicking a tag works the same way.

**Detected Colors** shows the main colors Pigoune found in the asset. To keep only the assets of a color, use the **Filters** button.

### Smart collections

A smart collection is a saved search that keeps itself up to date: it always shows the assets that match its criteria, including the ones you import later. For example, *all my favorite SVG files* or *every landscape image that fits my screen*.

It always searches the whole library, the trash left aside. Assets cannot be dropped onto a smart collection, since its content follows its criteria.

<img src="../../data/screenshots/07.png" alt="The window to create a smart collection, with a name, the words to find as pills and the filters" width="720">

#### Create a smart collection

1. Click the **+** button next to **Smart Collections** in the sidebar. The window is filled in with the search and the filters in use, and with **Favorites Only** when **Favorites** is open.
2. Give it a name.
3. Choose at least one criterion: words to find, types, orientations, colors, **Fits My Screen** or **Favorites Only**.

**Words to Find** works like the search field: spaces and commas, and pills whose **and** and **or** cut or join the groups. **Fits My Screen** follows the screen Pigoune is shown on when you open the smart collection.

#### Use and manage

| To… | Do this |
|---|---|
| **Open** it | Click it in the sidebar. You can still search inside it. |
| **Change** it | **Edit…** in its right-click menu, or <kbd>F2</kbd>. |
| **Delete** it | **Delete…**. Only the saved search goes: the assets stay in the library. |
| **Reorder** them | Drag them in the sidebar, or sort them by name or creation date with the **⋯** button next to their title. |

*See also: [Organizing](#organizing), [Looking at assets](#looking-at-assets).* · [Back to contents](#contents)

## Looking at assets

Browse the grid, then open the detailed preview to examine an asset closely.

### The grid

- **Size**: change the thumbnail size with the slider above the grid, or with <kbd>Ctrl</kbd>+<kbd>+</kbd> and <kbd>Ctrl</kbd>+<kbd>-</kbd>. The slider sets the smallest size: thumbnails grow slightly so that each row fills the whole width of the window.
- **Sort**: use the sort button to order the grid by date added, name, type, dimensions or size.
- **Animations**: animated GIF, PNG and WebP images play when you hover over them, unless your system is set to reduce animations.

### The detailed preview

Press <kbd>Space</kbd> or double-click an asset to open the detailed preview. A double-click opens the default application instead if you turned that setting on in **Preferences**. Press <kbd>Space</kbd> or <kbd>Esc</kbd> to go back.

<img src="../../data/screenshots/03.png" alt="The detailed preview of a rocket illustration, with a strip of thumbnails at the bottom" width="720">

#### Zoom and move

| To… | Do this |
|---|---|
| **Zoom** | Use the mouse wheel, or <kbd>+</kbd> and <kbd>-</kbd>. |
| **Fit the window** | Press <kbd>0</kbd>. |
| **Show the actual size** | Press <kbd>1</kbd>. |
| **Move a zoomed image** | Drag it to examine any part of it, even a corner brought to the middle of the screen, or use <kbd>Shift</kbd> and the arrow keys. |
| **Change the background** | Use the **Background** button of the top bar. |

#### Go from one asset to the next

- Use the arrow keys, the arrow buttons that appear when you move the mouse, or a two-finger swipe. <kbd>Home</kbd> and <kbd>End</kbd> jump to the first and last asset.
- A strip of thumbnails at the bottom shows the neighboring assets: click one to show it. It is hidden in full screen and in narrow windows. Turn it off with **Show Thumbnail Strip** in the zoom menu.
- Press <kbd>F11</kbd>, or choose **Full Screen** in the zoom menu, to fill the screen. The top bar comes back when you move the mouse or press <kbd>Tab</kbd>, and <kbd>Esc</kbd> leaves full screen.

#### Act on the asset

- The **Open With…**, **Copy** and **Export To…** buttons of the top bar act on the asset shown.
- **Show Details** opens the details panel next to the asset, to tag it, file it or add a note without leaving the preview. Pigoune remembers whether it is open.
- The star next to the back button adds the asset to your favorites.
- <kbd>Delete</kbd> moves the asset to the trash, and the preview goes on with the next one.

A right-click on the image offers **Open With…**, **Show in Files**, **Copy**, **Export To…**, **Convert and Export…**, **Frame and Set as Wallpaper…**, **Add to Favorites** and **Move to Trash**.

#### Check icons and pixels

- A dashed outline shows the real edges of the image, transparent margins included. Turn it off with **Show Image Bounds** in the zoom menu.
- From 800% on, a light grid separates the pixels of images, which helps to check icons and pixel art. Turn it off with **Show Pixel Grid** in the zoom menu.
- For an ICO file holding several sizes, buttons below the image show each size as it was drawn (16, 32, 48, 256…).

#### Animations

For an animation, a bar below the image pauses and resumes it, steps through it frame by frame, and shows which frame is displayed.

From the keyboard, <kbd>K</kbd> pauses or resumes, and <kbd>,</kbd> and <kbd>.</kbd> show the previous and next frame.

*See also: [Reusing assets](#reusing-assets), [Keyboard shortcuts](#keyboard-shortcuts).* · [Back to contents](#contents)

## Reusing assets

Take your assets out of Pigoune, or turn them into something else: a copy, an export, a wallpaper.

### At a glance

| I want to… | How | Details |
|---|---|---|
| Use an asset in another application | Drag it from the grid | [Drag and copy](#drag-and-copy) |
| Copy an asset | <kbd>Ctrl</kbd>+<kbd>C</kbd>, then paste | [Drag and copy](#drag-and-copy) |
| Save assets in a folder | **Export To…** | [Export to a folder](#export-to-a-folder) |
| Save the whole library as folders | **Manage Library**, **Export** tab | [Export to a folder](#export-to-a-folder) |
| Change the format or the size | **Convert and Export…** | [Convert and export](#convert-and-export) |
| Edit an asset in another application | **Open With…** | [Open and locate](#open-and-locate) |
| See the file on disk | **Show in Files** | [Open and locate](#open-and-locate) |
| Find look-alikes | **Find Similar…** | [Find similar assets](#find-similar-assets) |
| Make a wallpaper | **Frame and Set as Wallpaper…** | [Wallpapers](#wallpapers) |

Except for dragging, copying and exporting the whole library, these actions are in the right-click menu of an asset.

### Drag and copy

Drag assets from the grid into any application: a text editor, a design tool, a web page… Pigoune hands over a copy named after the asset.

To copy assets instead, press <kbd>Ctrl</kbd>+<kbd>C</kbd> and paste them elsewhere.

<details>
<summary>If a move pointer appears</summary>

Some applications, such as Files, may show a move pointer. It is the copy handed over that moves, never the asset in your library.

</details>

### Export to a folder

Choose **Export To…** in the right-click menu, then a folder. Existing files are never overwritten.

To export the whole library as ordinary folders, use the **Export** tab of **Manage Library** (see [Export the whole library](#export-the-whole-library)).

### Convert and export

Change the format and the size while you export. Choose **Convert and Export…** in the right-click menu, then:

1. Pick PNG, JPEG, WebP, AVIF or ICO.
2. Set the options below.
3. Choose a folder.

<img src="../../data/screenshots/04.png" alt="The Convert and Export window set to the ICO format, with the sizes to include" width="720">

#### Format options

| Option | Effect |
|---|---|
| **Quality** | JPEG and AVIF offer a quality setting. |
| **Background** | A background color fills the transparent areas. JPEG always uses one, since it has no transparency. |
| **Icon Sizes** | Tick the sizes to include (16, 32, 48 and 256 by default). They all go into a single icon file. |

The other formats keep transparency, unless you turn off **Keep Transparency** to use a background color too.

#### Size options

| Option | Effect |
|---|---|
| **Width and height** | In pixels or as a percentage. 100% keeps the original size. |
| **Keep Proportions** (the padlock) | Closed, proportions are kept: changing one side updates the other. Open it to stretch an image freely. |
| **Size of My Screen** | Fits several images to your screen at once: each image takes exactly the size of the screen Pigoune is shown on, for example 1920 × 1080. |
| **Fill the Screen** | Centers the image, covers the whole screen and cuts what overflows. |
| **Whole Image** | Keeps the whole image and fills the empty bands with the background color, or leaves them transparent when transparency is kept. |

With the padlock closed, each image keeps its proportions and fits inside the given width and height, even when the images do not all have the same shape.

<details>
<summary>Good to know</summary>

- SVG images stay sharp at any size; an enlarged pixel image becomes blurry.
- A resized file carries its size in its name, such as `logo-512x384.png`.
- From the preview, an animation paused on a frame exports that frame, named for instance `spinner-frame-3.png`.
- Pigoune remembers your format, quality, background and unit for the next export, while the size always starts from the original.
- The original stays untouched in the library, and existing files are never overwritten.

</details>

### Open and locate

| To… | Do this |
|---|---|
| **Open** an asset in another application, such as an image editor | Choose **Open With…**. The application receives a copy. |
| **Find the file on disk** | Choose **Show in Files**. Your file manager opens on the file stored in the library. For an annotated image, it opens on a copy of the annotated image. |
| **Find the untouched original** of an annotated image | Choose **Show Original**. Your file manager opens on the original stored in the library. |

Your library stays untouched: save your changes under a new name and import them if you want to keep them.

> [!WARNING]
> The file shown by **Show Original**, and by **Show in Files** for an image that is not annotated, is the real file. Do not move, rename or edit it from there: the check in the **Health** tab would point it out. The copy of an annotated image that **Show in Files** shows is only a copy: Pigoune does not use it.

### Find similar assets

Choose **Find Similar…** in the right-click menu of a single asset. A window lists the assets that look like it, closest first, even in another format or size.

Each one is marked **Almost identical**, **Very close** or **Similar**. Within each mark, the assets whose colors are closest come first. Choosing one shows it in All.

> [!NOTE]
> If the asset has not been analyzed yet, or its image is too plain to compare (a single color, or nothing visible), the window says so.

### Wallpapers

Frame any image and set it as your wallpaper. Choose **Frame and Set as Wallpaper…** in the right-click menu of the grid or of the preview.

#### Frame the image

A large window shows a virtual screen at the real resolution of the screen Pigoune is shown on, for example 1920 × 1080, with the image framed the way GNOME would.

| To… | Do this |
|---|---|
| **Move** the image | Drag it. |
| **Zoom** | Use the mouse wheel, the slider, or <kbd>+</kbd> and <kbd>-</kbd>. |
| **Adjust** | Use the arrow keys (<kbd>Shift</kbd> moves further). |

Three buttons set the framing:

| Button | Effect |
|---|---|
| **Fill** | Returns to GNOME's own framing. |
| **Whole** | Shows all of the image. |
| **100%** | Shows one pixel of the image per pixel of the screen. |

<img src="../../data/screenshots/09.png" alt="The window to frame a wallpaper: a virtual screen with a portrait image on a blurred background and its controls" width="720">

#### Helpers

| Option | Effect |
|---|---|
| **Mirror** | Flips the image from left to right, in the wallpaper only. |
| **Rule of Thirds** | Shows the grid of thirds, to place the subject. |
| **Snap** | On by default: the dragged image catches the center and the edges of the screen, a blue line showing where. |

What goes past the screen stays visible, faded. When the image is enlarged past 100%, a notice over the screen warns that it will look blurry.

The full screen button, or <kbd>F11</kbd>, shows the virtual screen at its real size. A short notice reminds you that it is a simulation. <kbd>Esc</kbd> comes back.

#### Image and screens

| Option | Effect |
|---|---|
| **Darken** (under **Image**) | Darkens the whole wallpaper, up to 60%, so that icons and bars stay readable. |
| **Prepare For** | With several screens, chooses the one the wallpaper is made for. GNOME shows the same wallpaper on every screen and adapts it to the others. |
| **Desktop Simulation** | Draws an imitation of the bars of your desktop over the virtual screen. Turn it off to see the image alone. |

Like **Mirror**, **Darken** changes the wallpaper only, never the asset.

The simulation imitates GNOME, KDE Plasma, Cinnamon, Xfce, MATE, COSMIC or Budgie; the one you use is selected by default.

#### Empty space

Under **Empty Space**, choose what fills the screen around a smaller image.

| Choice | What it does |
|---|---|
| **Color** | Black by default. Pick one of the main colors of the image, or black or white. |
| **Gradient** | Starts from the two main colors of the image, from top to bottom. |
| **Blur** | The same image, enlarged and blurred behind it. |
| **Mosaic** | The image repeated over the whole screen, handy for patterns. |

The rainbow swatch opens the GNOME color chooser.

A gradient has three more settings:

| Setting | Effect |
|---|---|
| **Angle** | Rotates the gradient (0° goes up, 90° to the right, 180° down). |
| **Start Color** and **End Color** | Open the GNOME color chooser. |
| **Swap Colors** | Exchanges the two colors. |

#### Set the wallpaper

**Set as Wallpaper** makes an image the size of the screen, so the desktop shows what you framed. A small window shows the preparation, then GNOME asks you to confirm.

Tick **Also Add the Edited Version to the Library** to keep that image as a new asset.

*See also: [Capturing and annotating](#capturing-and-annotating), [Your library](#your-library).* · [Back to contents](#contents)

## Capturing and annotating

Take a screenshot, paste an image or open one from your library, then annotate it with shapes, text, arrows, blur and more.

The editor opens inside the window of an open library, so there is always a place to keep the result.

> [!NOTE]
> Annotations never change the original file. They are kept next to it, and you can change them later.

<img src="../../data/screenshots/12.png" alt="The annotation editor on a screenshot, with a dashed rectangle, an arrow, a text, a numbered step and a blurred email address" width="720">

### Start a capture

#### Take a screenshot

Choose **Take a Screenshot** in the import menu, press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd>, or use the **Take a Screenshot** action of the Pigoune icon (right-click it in your launcher).

Pigoune hides its window, and your desktop asks you to choose the area, a window or the whole screen.

#### Paste an image

Copy an image anywhere, then choose **Paste and Annotate** in the import menu or press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>V</kbd>.

#### Annotate an image of your library

Select the image and choose **Annotate…** in the details panel or the context menu, or press <kbd>Ctrl</kbd>+<kbd>E</kbd>.

#### Start from a desktop shortcut

Bind the command `pigoune --capture` (or the full Flatpak command below) to a key in the settings of your desktop:

```sh
flatpak run io.github.gor3pig.Pigoune --capture
```

If Pigoune is not running, it starts, reopens your last library and takes the screenshot.

An image to annotate (a screenshot, a pasted image or an image from your library) can be up to 8000 pixels on a side.

### Annotate

The tools are in the editor panel, on the right (in a narrow window, in the panel that rises from the bottom). Each tool has a letter that selects it, shown in its tooltip.

| Tool | Key | What it does |
|---|---|---|
| Select | <kbd>V</kbd> | Select, move and resize an annotation |
| Crop | <kbd>C</kbd> | Keep only a part of the image |
| Rectangle, Ellipse | <kbd>R</kbd>, <kbd>O</kbd> | Outlines, filled or with rounded corners |
| Line, Arrow | <kbd>L</kbd>, <kbd>A</kbd> | Straight or curved, with several heads |
| Text | <kbd>T</kbd> | Add text: click, type, then click elsewhere or press <kbd>Ctrl</kbd>+<kbd>Enter</kbd> to finish |
| Bubble | <kbd>U</kbd> | A comic speech or thought bubble with a movable tail and centered text |
| Steps | <kbd>N</kbd> | Numbered (or lettered) badges that renumber themselves |
| Pen, Highlighter | <kbd>P</kbd>, <kbd>H</kbd> | Smoothed freehand strokes |
| Blur, Pixelate | <kbd>B</kbd>, <kbd>X</kbd> | Hide private information: the effect applies to everything drawn below it |
| Spotlight | <kbd>S</kbd> | Dims everything but a rectangle, a rounded rectangle or an oval |

<kbd>Delete</kbd> removes the selected annotation. **Crop** works with free or fixed proportions.

#### Settings of a tool

Every tool remembers its last settings until Pigoune is closed: color (with an eyedropper to take one from the image), opacity, thickness, shadow and more. The text tool also has font, size, bold, italic, background, bubble and outline.

- **Line Type** chooses a solid, dashed or dotted stroke, for rectangles, ellipses, lines, arrows and the pen.
- **Hand-Drawn Look** (Precise, Sketchy or Very Sketchy) gives rectangles, ellipses, lines and arrows the slightly wobbly look of a drawing made by hand.
- A shape keeps the same look every time you open it, and a copy looks exactly like the original.
- Arrows can be even or tapered.

#### Speech bubbles

- Drag to draw the bubble, then type: the text is centered and wraps by itself. Double-click a bubble to edit its text.
- **Bubble Shape** chooses an oval, a rounded rectangle or a cloud for a thought, and **Cloud Bumps** sets the size of the cloud's bumps.
- Drag the round handle at the tip of the tail to aim it anywhere. It stops one bubble size away from the edge.
- **Bubble Lettering** chooses a comic font or the normal font, and **Bubble Fill** the color inside.
- Turn on **Shadow** and pick a soft or a hard one with **Shadow Style**. A dashed **Line Type** makes a whisper.

#### Draw precisely

- Hold <kbd>Shift</kbd> while drawing for straight lines at multiples of 45 degrees, and for perfect squares or circles.
- Hold <kbd>Alt</kbd> while moving to switch off the guides that align an annotation with the image and with the other annotations.
- Undo and redo work as everywhere: <kbd>Ctrl</kbd>+<kbd>Z</kbd> and <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd>.

#### Select and edit

A new annotation stays selected: you can move it, resize it by its handles and change its settings right away, without leaving the tool you are using.

- With a drawing tool, click the outline of an annotation to select it, or anywhere else, even inside another annotation, to draw. The **Select** tool selects an annotation by its inside too.
- A selected arrow has a round handle in the middle: drag it to bend the arrow, double-click it to straighten the arrow.
- A selected bubble has a round handle at the tip of its tail.
- A selected blur, pixelation or spotlight has a round handle in its center: drag it to move the area with any tool.
- The arrow keys nudge the selection by one pixel (ten with <kbd>Shift</kbd>).
- <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>D</kbd> duplicates it.

#### Resize and turn

- A selected **text** has handles like the other annotations: dragging one scales the letters, and the opposite corner stays where it is.
- The round handle above the frame of a selected annotation **turns** it. Hold <kbd>Shift</kbd> to turn by steps of 15 degrees. A step badge or a spotlight does not turn.
- A rectangle, an ellipse, a bubble, a text, a blur and a pixelation also have a **Rotation** setting in the panel, from -180 to 180 degrees, for an exact angle.
- A line, an arrow, a pen stroke or a highlighter turns around the center of its frame, and has no angle setting.
- The frame and the handles turn with the annotation, and a click selects it where you see it.

### Layers

Every annotation is a layer. The **Layers** panel lists them from front to back.

<img src="../../data/screenshots/13.png" alt="The editor in dark style with the Layers panel listing a step, a text, an arrow, a rectangle and a blur" width="720">

| To… | Do this |
|---|---|
| **Change the order** | Use the buttons above the list, or drag a row by its grip or its name. |
| **Hide or show** | Click the eye of a row. |
| **Lock** | Click the padlock, so it cannot be moved by mistake. |
| **Rename** | Double-click the name, click it again after a pause on the selected layer, or right-click the row. |
| **Duplicate, delete** | Right-click a row. |

<kbd>Ctrl</kbd>+<kbd>]</kbd> and <kbd>Ctrl</kbd>+<kbd>[</kbd> change the order too, with <kbd>Shift</kbd> for the extremes.

A hidden layer disappears from the editor and from the result, but it is not deleted: you can show it again at any time.

### Your workspace

**Tools**, **Properties** and **Layers** share the panel on the right and are always shown, one under the other. The tools are a small grid of icons: hover one to see its name and its shortcut. Drag the handle between Properties and Layers to give one more height.

The button next to **Redo** in the header, or <kbd>F9</kbd>, hides the whole editor panel to give the image more room. **Save** and the copy, export and capture buttons then move to the header.

Pigoune remembers whether the panel is hidden. None of this changes your annotations, the selected tool or the zoom.

### Hide what is private

A **Blur** or a **Pixelate** annotation hides what is under it, annotations included. Like every annotation, it never destroys anything: the original file stays as it was imported. It is saved with the other annotations and stays editable: you can move it, resize it or hide it later.

> [!WARNING]
> The original image stays in your library, with what you hid. A pixelation with large blocks removes the details it covers, but a light blur is not an anonymization and may be partly recoverable. For anything sensitive, prefer a pixelation or a strong blur, and do not share the original file.

Pigoune takes this into account:

- If a blur or a pixelation is hidden (with the eye of its layer), some information may be visible again in the result. Pigoune warns you before you **copy**, **export** or **open with another application** an image that has one, and offers to show it first when you are in the editor.
- Dragging such an image out of Pigoune is refused until you show the hidden effects again, with no option to continue anyway.
- Exporting the whole library gives every original **and** a copy of each annotated image, so the originals may contain things that are no longer visible on the annotated copies. Pigoune asks whether you want the originals too before it exports.
- **Show in Files** does not ask this question: it reveals a copy of the annotated image in your file manager, where it can then be opened or copied. Check the hidden effects first if that matters.

### Keep the result

- **Save** adds the annotated image to your library under a name based on the date, with the `capture` tag, and shows it selected. An image that is already in the library is refused, and the message names the existing asset.
- The menu next to **Save** copies the annotated image to the clipboard or exports it as a PNG.
- If Pigoune closes while you edit, the capture is kept as a draft, and Pigoune offers to resume it the next time a library opens.

#### Reopen the annotations later

Select an annotated asset and choose **Annotate…** (in the details panel or the context menu). Saving updates the same asset. **Show Original** opens your file manager on the untouched image stored in the library.

The grid, the preview and the duplicates use the annotated image. Copying or exporting it gives a flattened PNG. The original stays next to it: it follows the annotated image to the trash and back, and counts in the storage of your library.

### Compare with the original

In the detailed preview, an annotated image shows an eye button at the bottom right. **Hold** it down to see the original image, and release it: the annotated image comes back at once.

It is a momentary view, not a switch, so nothing stays on screen by mistake: the annotated image also comes back if you change image, close the preview or leave the window.

Nothing is changed or saved. The button is dimmed if the original file is not available.

*See also: [Reusing assets](#reusing-assets), [Keyboard shortcuts](#keyboard-shortcuts).* · [Back to contents](#contents)

## Trash and undo

Nothing is lost until you empty the trash, and most changes can be undone.

### The trash

Press <kbd>Delete</kbd>, or choose **Move to Trash** in the right-click menu, to send assets to the trash.

Open **Trash** at the bottom of the sidebar to restore assets, with **Restore** in the right-click menu or in the details panel, or to empty it for good.

Assets in the trash can still be copied, exported or opened with another application, but not changed.

### Undo and redo

| To… | Do this |
|---|---|
| **Undo** | Press <kbd>Ctrl</kbd>+<kbd>Z</kbd>, click **Undo** at the top of the sidebar, or click **Undo** in the message that appears after an action. |
| **Redo** | Press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd> (or <kbd>Ctrl</kbd>+<kbd>Y</kbd>), or click **Redo** next to **Undo** or in the message that follows an undo. |

You can redo a change that was undone until you make another change.

#### What can be undone

- Trashing, moving, tags, favorites, renaming, notes and credits.
- Creating, renaming, customizing, moving or deleting a collection.
- Creating, editing, reordering or deleting a smart collection.

#### Good to know

- Imports cannot be undone, and emptying the trash clears the undo history.
- If an import takes an asset out of the trash, the earlier "Move to Trash" of that asset is no longer in the undo history.
- Creating a collection is only undone while it is still empty. Once assets are imported into it, use **Delete…** instead.
- When an undo is no longer possible, Pigoune says so and leaves that change as it is. The next <kbd>Ctrl</kbd>+<kbd>Z</kbd> undoes the change before it.
- A message that follows an action counts only what really changed: assets that were already in the collection or already had the tag are not counted.

*See also: [Organizing](#organizing), [Your library](#your-library).* · [Back to contents](#contents)

## Your library

A library is a regular folder whose name ends with `.pigoune`. Everything Pigoune keeps for you lives inside it.

### What is inside

- `files/`: a copy of every asset, with its original file name, sorted into small subfolders so that even very large libraries stay easy to handle.
- `library.db`: the database holding collections, tags and all other information.
- `cache/`: the thumbnails and the analysis data of the images, which Pigoune can rebuild at any time.

### Move and back up

Because everything is inside this folder, a library is **portable**: copy it to an external drive or to another computer and open it there with Pigoune. To back it up, copy the whole folder while Pigoune is closed.

A library can only be open in one Pigoune window at a time. If you keep a library in a synced folder (Nextcloud, Syncthing…), close Pigoune on one computer before opening it on another.

> [!NOTE]
> A library that has been opened and upgraded with Pigoune 3.1 can no longer be opened with Pigoune 3.0 or older. **Manage Library** shows which versions can open it.

### Manage Library

Choose **Manage Library** in the main menu to see what the open library holds and to look after it. The window has six tabs.

| Tab | What it is for |
|---|---|
| **Overview** | See the numbers and records of the library |
| **Content** | See how formats and colors are shared |
| **Storage** | See what the library takes on your disk |
| **Health** | Check the files against the database |
| **Duplicates** | Find assets that look alike |
| **Export** | Copy every asset into folders |

<img src="../../data/screenshots/05.png" alt="The Overview tab of Manage Library in dark style, with the number of assets, collections, tags and favorites, and the records" width="720">

#### Overview

The name and location of the library, the number of assets, collections, tags and favorites, and the **Records**: the heaviest, the largest, the newest and the oldest asset. Click one to see it in the grid.

#### Content

A ring chart shows how the space is shared between formats, by size or by number of assets. The tab also shows the number of animated images, SVG images and assets in the trash, and the detected colors with the number of assets where each one is a main color.

#### Storage

A single bar shows what the library takes on its disk, split between the assets, the thumbnails, the database and the trash, next to the other files and the free space.

Below are the location of the library (with buttons to copy it or open its folder), whether that disk is removable, the space taken by the thumbnails, when the library was created and which versions of Pigoune can open it.

**Clear** next to the thumbnails frees their space. Pigoune makes them again quietly in the background, while you are not using it.

### Check the health of your library

The **Health** tab compares the files on the disk with the library's database.

<img src="../../data/screenshots/11.png" alt="The Health tab listing a missing file, a damaged file and an unregistered file, each with its buttons" width="720">

**Check** reads every file, except those in the trash, and lists:

- the **missing** files;
- the **damaged** files, whose content no longer matches what was imported;
- the **unregistered** files, which are in the library folder but unknown to the database.

The check runs in the background and can be canceled. It never changes or deletes anything by itself. Assets that are in the trash are not reported: their files leave with the trash, and if you restore one, the next check looks at it again.

#### Fix a problem

- **Add** (or **Add All**) puts an unregistered file back in the library, in Unclassified, without moving the file. Pigoune explains why when a file cannot be added, for example when it is not an image.
- **Replace…** lets you choose a sound copy of a missing or damaged file. Pigoune accepts it only if its content is identical to what was imported, then puts it into the library and keeps the asset's name, tags and collections.
- **Remove from the Library** removes a missing asset when no copy is left, after a confirmation that says it cannot be undone. A damaged file, which still exists, never offers it.
- **Move to Trash** is offered for a damaged file instead. You can undo it from the message that appears.

### Find duplicates

The **Duplicates** tab groups the assets that look alike, such as the same logo as a PNG and an SVG, or in two sizes.

Pigoune analyzes the image of every asset quietly in the background, only while you are not using it. The Overview tab shows how far it has got, and the Duplicates tab says how many assets are still waiting.

#### Run the search

Click **Search**. Pigoune first analyzes in detail, once, the assets that could be duplicates, then compares them, with a progress bar and a **Cancel** button. It lists the groups; in each one, the largest image comes first.

- A group shows its first six assets. **Show More** adds twenty at a time, and the thumbnails load as you scroll.
- Each group is only a suggestion: the assets look alike, which does not make them identical.
- Pigoune compares the images in two steps, then marks each asset **Identical** or **Probable** against the largest one. A group only holds assets that all look alike, and holds 40 assets at most.

#### Clean up a group

1. Pick the asset to keep with its round button. The largest image is picked first, and this choice deletes nothing.
2. Tick the box (“Move to Trash”) on each asset you want to remove. Nothing is ticked for you.
3. Click **Move N Assets to the Trash…**, which appears once something is ticked. A summary shows what is kept and what goes to the trash, with the collections, tags and notes that stay behind.
4. Confirm. Only the assets you ticked are moved, and **Cancel** changes nothing.

A message lets you undo the move: the assets stay in the trash until you empty it.

If the assets changed since the search, Pigoune moves nothing and searches again.

Choosing a row shows that asset in All.

#### Ignore look-alikes that are not duplicates

- **Ignore Group** in the “⋯” menu of a group, or **Not a Duplicate** in the “⋯” menu of one row, stops Pigoune from offering those assets as duplicates of each other. The message lets you undo it.
- **Ignored (N)**, at the top of the tab, lists what you ignored. Each entry is either a group or a single asset with the assets it is kept apart from.
- **Stop Ignoring** undoes exactly one entry, and **Show All Again…** all of them.

The **Ignored** button stays there when you reopen **Manage Library**, and **Undo** (<kbd>Ctrl</kbd>+<kbd>Z</kbd>) updates the list.

<details>
<summary>How ignoring works</summary>

Ignoring a group only concerns the assets of that group. They are no longer offered with each other, nor with the other assets that look like them, and those other assets are still offered in their own groups. A new image that looks like one of them is still offered with that one.

</details>

#### Leave a collection out

- Right-click a collection in the sidebar and choose **Leave Out of Duplicate Search**, then confirm. Its sub-collections follow.
- The **Left Out** button of the Duplicates tab lists the collections left out, with where each one sits and how many sub-collections go with it. **Include** puts a collection back, and **Undo** works too.
- **Include in Duplicate Search** in the right-click menu does the same from the sidebar. For a sub-collection left out through its parent, the menu names that parent.
- An asset is left out only when **every** collection it belongs to is left out. An asset in no collection, or in another one, is still compared.
- The Duplicates tab also says how many collections are left out. Similar assets and the duplicates spotted at import do not change.

### Export the whole library

The **Export** tab copies every asset into a folder you choose.

- Pigoune creates a folder named after the library, with one sub-folder for each collection and sub-collection, and puts the original files inside. The tab shows an example made from your own library.
- Assets that belong to no collection go into an **Unclassified** folder. An asset that belongs to several collections is copied into each of them. Assets in the trash are left out.
- An annotated image is exported as its original **and** a copy of the annotated image. See [Hide what is private](#hide-what-is-private).
- Nothing is ever overwritten: if a folder with that name already exists, the export is created as `Name (2)`. The button is disabled when the library is empty.

> [!NOTE]
> Tags, notes and favorites stay in Pigoune, so this is not a full backup. For that, copy the library folder.

*See also: [Preferences](#preferences), [Reusing assets](#reusing-assets).* · [Back to contents](#contents)

## Preferences

Open **Preferences** from the main menu, or press <kbd>Ctrl</kbd>+<kbd>,</kbd>. The settings are split into four pages. The magnifier at the top finds a setting by its name.

### General

| Setting | What it does |
|---|---|
| **Interface Language** | Follows your system, or choose another language. It applies at the next start; **Restart** does it at once. A few windows provided by GNOME, such as the one to choose a folder, may stay in the language of the system. |
| **Reopen the Last Library** | At startup, opens the library you left open. Turn it off to start from the welcome page. |
| **Recent Libraries** | How many recent libraries the main menu and the welcome page offer, from 0 to 8. Choose 0 to turn the list off. |
| **Reopen the Last Entry** | A library opens on the entry you used last instead of **All**. |
| **Show What’s New After an Update** | A short window lists the main news the first time a new version starts. The About window keeps the news of the current version. |
| **Confirm Before Emptying the Trash** | Asks you to confirm before the trash is emptied. |
| **Empty the Trash Automatically** | Assets are deleted for good after 30 days in the trash. Off by default. |

### Display

#### Grid options

| Setting | What it does |
|---|---|
| **Show Asset Names** | Shows the name under each thumbnail. When names are hidden, hover over a thumbnail to see its name. |
| **Clickable Asset Names** | Shown only when names are shown. Turn it off and the name counts as empty space. |
| **Show Formats** | A badge shows the format of each asset (SVG, PNG…) on its thumbnail. |
| **Thumbnail Background** | White, gray, black or a checkerboard, to see white or black images and transparent areas. |
| **Play Animations on Hover** | Turn it off if moving thumbnails distract you. Animations still play in the details panel and the preview. |

When names are not clickable, only the thumbnail reacts to clicks, double-clicks and drags, and a selection rectangle can start from the name.

#### Sidebar options

| Setting | What it does |
|---|---|
| **Show Asset Counts** | Shows a count next to each entry. While you search or filter, only matching assets are counted. |
| **Show Tags** | Turn it off to hide the tags section. Tags still appear in the details panel, and searches still find them. |
| **Show Smart Collections** | Turn it off to hide that section. Your smart collections are kept and come back when you turn it on again. |

Tags with no matching asset are dimmed while you search or filter.

### Behavior

| Setting | What it does |
|---|---|
| **Open Assets on Double-Click** | A double-click opens the asset in its default application instead of the preview. <kbd>Space</kbd> still opens the preview. |
| **Search the Whole Library** | Searches and filters look everywhere, not only in the selected entry, except in the trash and in smart collections. |

**Search the Whole Library** is on at the first start, and the label in the search field changes it at any time.

### Shortcuts

Most of the shortcuts listed in [Keyboard shortcuts](#keyboard-shortcuts) can be changed here. Select an action, then press the new key combination.

- Pigoune refuses a combination already used by another action, and tells you which one.
- It also refuses a single letter, because typing in the grid starts a search (use <kbd>Ctrl</kbd> or <kbd>Alt</kbd> with a key, or a function key), and the keys it keeps for itself, such as <kbd>F2</kbd>, <kbd>F10</kbd> and <kbd>F11</kbd>.
- A button next to a changed shortcut brings back its default, and **Restore All Shortcuts** brings back all of them.
- The standard shortcuts (copy, paste, select all, undo, redo and quit) are shown with a lock and cannot be changed.
- Some actions, such as importing a folder, exporting or restoring assets, have no shortcut until you give them one.

### Style and colors

The three round buttons at the top of the main menu choose the style: **Follow the System** (the default), **Light Theme** or **Dark Theme**. The choice applies at once and is remembered. The welcome page has them too.

The accent color is not set in Preferences: Pigoune follows the one you choose in GNOME **Settings**, under **Appearance**. It colors selections, switches and highlights.

*See also: [Keyboard shortcuts](#keyboard-shortcuts), [Your library](#your-library).* · [Back to contents](#contents)

## Keyboard shortcuts

Press <kbd>Ctrl</kbd>+<kbd>?</kbd> to see every shortcut in Pigoune. Most of them can be changed in **Preferences**, on the **Shortcuts** page.

### Libraries and import

| Action | Shortcut |
|---|---|
| New library | <kbd>Ctrl</kbd>+<kbd>N</kbd> |
| Open a library | <kbd>Ctrl</kbd>+<kbd>O</kbd> |
| Import files | <kbd>Ctrl</kbd>+<kbd>I</kbd> |
| Paste files or an image to import | <kbd>Ctrl</kbd>+<kbd>V</kbd> |

### Browsing

| Action | Shortcut |
|---|---|
| Search | <kbd>Ctrl</kbd>+<kbd>F</kbd> |
| Detailed preview | <kbd>Space</kbd> or <kbd>Enter</kbd> |
| Select all / deselect all | <kbd>Ctrl</kbd>+<kbd>A</kbd> / <kbd>Esc</kbd> |
| Copy | <kbd>Ctrl</kbd>+<kbd>C</kbd> |
| Larger / smaller thumbnails | <kbd>Ctrl</kbd>+<kbd>+</kbd> / <kbd>Ctrl</kbd>+<kbd>-</kbd> |
| Context menu | <kbd>Menu</kbd> or <kbd>Shift</kbd>+<kbd>F10</kbd> |

### Organizing assets

| Action | Shortcut |
|---|---|
| Rename the selected asset | <kbd>F2</kbd> |
| Add to or remove from favorites | <kbd>Ctrl</kbd>+<kbd>D</kbd> |
| Add a tag | <kbd>Ctrl</kbd>+<kbd>T</kbd> |
| Add to a collection | <kbd>Ctrl</kbd>+<kbd>K</kbd> |
| Move to another collection (from inside a collection) | <kbd>Ctrl</kbd>+<kbd>M</kbd> |
| Move to trash | <kbd>Delete</kbd> |

### Sidebar

| Action | Shortcut |
|---|---|
| Rename or edit the selected entry | <kbd>F2</kbd> |
| Delete the selected collection, smart collection or tag | <kbd>Delete</kbd> |
| Move between the entries | <kbd>Up</kbd> / <kbd>Down</kbd> |
| Expand or collapse an entry | <kbd>Right</kbd> / <kbd>Left</kbd> |

### Annotating

| Action | Shortcut |
|---|---|
| Take a screenshot to annotate | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> |
| Paste an image to annotate | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>V</kbd> |
| Annotate the selected image | <kbd>Ctrl</kbd>+<kbd>E</kbd> |
| Show or hide the editor panel | <kbd>F9</kbd> |

The letters of the tools are listed in [Annotate](#annotate).

### Everywhere

| Action | Shortcut |
|---|---|
| Undo | <kbd>Ctrl</kbd>+<kbd>Z</kbd> |
| Redo | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd> or <kbd>Ctrl</kbd>+<kbd>Y</kbd> |
| Help | <kbd>F1</kbd> |
| Preferences | <kbd>Ctrl</kbd>+<kbd>,</kbd> |
| Close the window | <kbd>Ctrl</kbd>+<kbd>W</kbd> |
| Quit | <kbd>Ctrl</kbd>+<kbd>Q</kbd> |

*See also: [Preferences](#preferences).* · [Back to contents](#contents)

## Questions and answers

### Can I delete my original files after importing them?

Yes. Pigoune works on its own copies, stored inside the library.

### Does annotating an image change my original?

No. Annotations, blur and pixelation included, are kept next to the original, which stays exactly as it was imported. See [Hide what is private](#hide-what-is-private) before sharing an annotated image.

### Does Pigoune send anything over the internet?

No. Pigoune works fully offline, with no account and no telemetry. Checking for updates is done by Flatpak, which only asks the Pigoune repository whether a new version exists.

### How do I update Pigoune?

The Software app takes care of it: it offers each new version, and installs it by itself when automatic updates are turned on. `flatpak update` installs it too.

If Pigoune stays open, a banner at the top of the window also says so within half an hour. Click **Update**, wait for the installation, then click **Restart**.

### Why does Pigoune say a file is unreadable?

The file is damaged, or is not really in the format its name suggests. Pigoune checks the content of each image, not just its extension.

### I imported the same file twice. Where is the second copy?

There is none. Pigoune recognizes duplicates and adds the existing asset to the target collection instead.

### How do I report a bug or suggest an idea?

Open an issue on [GitHub](https://github.com/Gor3pig/Pigoune/issues).

*See also: [Getting started](#getting-started).* · [Back to contents](#contents)
