<p align="center">
  <img src="data/icons/pigoune-256x256.png" alt="Pigoune icon" width="160" height="160">
</p>

<h1 align="center">Pigoune</h1>

<div align="center">

**P**latform for **I**cons and **G**raphics **O**rganized, **U**nified, **N**ative and **E**legant.

</div>

<p align="center">
  <a href="README.md">Français</a> · English
</p>

Pigoune is a personal graphical asset library designed natively for Linux and GNOME. It aims to provide a reliable, visual home for organizing, preserving and finding icons, logos, illustrations and other graphical resources, entirely locally and offline.

## Project status

Pigoune is in early development. The Rust, GTK 4, Libadwaita and Flatpak foundations are in place, but no stable release has been published yet and the product interface remains to be built.

## Why Pigoune?

Graphical resources often end up scattered across file hierarchies that are difficult to browse or tied to their original location. Pigoune aims to provide a dedicated, native GNOME library where every imported asset becomes independent from its source file.

## Key principles

- local and offline operation, with no account, cloud, telemetry or analytics;
- bit-for-bit preservation of imported binary content;
- SHA-256 identification of physical content and a distinct identity for every logical asset;
- self-contained, portable libraries;
- data integrity, durability and recoverability take priority.

## Main 1.0 goals

The first stable release aims to provide a coherent foundation for importing and preserving assets, organizing and finding them, producing previews, exporting them, and protecting the library through integrity, backup and recovery mechanisms.

## Technologies

| Technology | Planned role |
| --- | --- |
| Rust | Application and domain core |
| GTK 4 and Libadwaita | Native GNOME interface |
| SQLite | Library metadata |
| Glycin | Image loading and decoding |
| Flatpak | Distribution and sandboxing |

## Architecture

The workspace separates `pigoune-core`, which holds the domain without graphical dependencies, from `pigoune-app`, the GNOME frontend. Exchanges between the two layers will remain explicit and testable.

## Documentation

- [Product specification — English translation](docs/product/specification.en.md)
- [Spécification produit — French canonical version](docs/product/specification.md)

## License

Pigoune is licensed under the [GNU General Public License version 3 or later](LICENSE).
