<div align="center">

<img src="data/icons/pigoune-256x256.png" alt="Pigoune" width="160" height="160">

# Pigoune

**P**latform for **I**cons and **G**raphics **O**rganized, **U**nified, **N**ative and **E**legant

*All your graphic assets, organized and within reach.*

[![License: GPL v3](https://img.shields.io/badge/license-GPL--3.0-3584e4?style=for-the-badge)](COPYING)
[![GNOME](https://img.shields.io/badge/GNOME-51-4a86cf?style=for-the-badge&logo=gnome&logoColor=white)](https://www.gnome.org)
[![Rust](https://img.shields.io/badge/Rust-GTK4%20%C2%B7%20libadwaita-e66100?style=for-the-badge&logo=rust&logoColor=white)](https://gtk-rs.org)
[![Latest release](https://img.shields.io/github/v/release/Gor3pig/Pigoune?style=for-the-badge&label=version&color=9141ac&logo=flatpak&logoColor=white)](https://github.com/Gor3pig/Pigoune/releases/latest)

**English** · [Français](README.fr.md)

[Website](https://gor3pig.github.io/Pigoune/) · [Features](#features) · [Principles](#principles) · [Installation](#installation) · [Guide](#user-guide) · [Contributing](#contributing)

<img src="data/screenshots/01.png" alt="The main window of Pigoune" width="860">

</div>

---

> [!NOTE]
> **Pigoune is available.** [Install it in one click](#installation), and it keeps itself up to
> date.

## What is Pigoune?

Icons in `Downloads`, logos in an old project folder, an SVG lost somewhere on the desktop…
**Pigoune gathers all of them in a single library**, neatly organized and pleasant to browse.

Drop your graphic assets in: Pigoune keeps a safe copy (your original files are never
touched), helps you organize them and hands them back with a simple drag and drop whenever
you need them.

## Features

<table>
<tr>
<td width="50%" valign="top">

### Drop
Drag files or whole folders: SVG, PNG, JPEG, WebP, AVIF, JPEG XL, GIF, TIFF, BMP and ICO. Every
image is checked before it comes in, and duplicates are recognized automatically.

</td>
<td width="50%" valign="top">

### Organize
Nested collections with their own icon and color, smart collections that fill themselves, tags
and favorites, plus a note, a source, a license and an author for each asset.

</td>
</tr>
<tr>
<td width="50%" valign="top">

### Find
Instant search and filters by type, orientation, color or favorite, even among thousands of assets, with
words combined by "and" or "or" in one click. Clicking a collection or a tag shows you where an asset
lives.

</td>
<td width="50%" valign="top">

### Reuse
Drag an asset into any application, open it with another one, copy it to the clipboard or
export it to a folder, as it is or converted to PNG, JPEG, WebP, AVIF or ICO.

### Wallpapers
Frame any image on a virtual copy of your screen, under the bars of your desktop, fill the gaps
with a color, a gradient, a blur or a mosaic, and set it: your desktop shows exactly what you
framed. Filters by orientation and by screen size find the images that fit.

</td>
</tr>
<tr>
<td width="50%" valign="top">

### Enjoy
A thumbnail grid with adjustable size, animations (GIF, PNG, WebP) on hover, and a detailed
preview with zoom, the details panel and a strip of thumbnails at the press of <kbd>Space</kbd>.

</td>
<td width="50%" valign="top">

### Relax
The trash and undo with <kbd>Ctrl</kbd>+<kbd>Z</kbd>: a mistake can always be fixed.

</td>
</tr>
</table>

## Principles

- **Your originals are never modified**: Pigoune works on its own copies.
- **Fully local**: no account, no cloud, no telemetry.
- **Portable libraries**: a plain folder you can keep anywhere and take to another computer.
- **Made for GNOME**: a native, polished app in light and dark mode, usable with the keyboard
  and a screen reader, and adaptive to small windows.

## Installation

### 1. Set up Flatpak

Pigoune is distributed as a Flatpak. Fedora, Linux Mint, Pop!_OS and many other distributions
ship Flatpak already. On Ubuntu and a few others, set it up first by following the
[official guide for your distribution](https://flathub.org/setup), then restart your session.

### 2. Install Pigoune

Open [pigoune.flatpakref](https://gor3pig.github.io/Pigoune/pigoune.flatpakref) with Software, which installs Pigoune from its own
repository, or run this command:

```sh
flatpak install --user https://gor3pig.github.io/Pigoune/pigoune.flatpakref
```

The GNOME runtime that Pigoune needs is downloaded automatically from Flathub.

### 3. Stay up to date

Pigoune updates itself like your other apps: Software offers each new version, and
`flatpak update` installs it too. If Pigoune stays open, a banner at the top of the window
also tells you within half an hour and offers to install the new version, then to restart. Every update is signed, so only
official versions are accepted.

If you installed Pigoune 1.5 or older from a downloaded `.flatpak` file, it does not update
itself. Switch once to automatic updates with these two commands. Your libraries and settings
are kept.

```sh
flatpak uninstall --user io.github.gor3pig.Pigoune
flatpak install --user https://gor3pig.github.io/Pigoune/pigoune.flatpakref
```

The `.flatpak` file of each version is still attached to its
[release](https://github.com/Gor3pig/Pigoune/releases). You can also build Pigoune yourself by
following [CONTRIBUTING.md](CONTRIBUTING.md).

## User guide

The [user guide](help/en/README.md) walks you through everything Pigoune can do. In the app, it
also opens from the main menu or with <kbd>F1</kbd>.

## Contributing

Pigoune is free software and help is welcome: bug reports, ideas, translations and code.
[CONTRIBUTING.md](CONTRIBUTING.md) explains how to build Pigoune and submit a change,
[ARCHITECTURE.md](ARCHITECTURE.md) gives an overview of the code and
[po/README.md](po/README.md) explains how to translate it.

## License

Pigoune is free software released under the [GNU GPL v3 or later](COPYING).
