# Pigoune — Product Specification

> [Français — reference version](specification.md) · English

**Version:** 0.1  
**Status:** Initial product baseline

## 1. Vision

Pigoune is a personal graphical asset library designed specifically for Linux and GNOME.

It allows users to collect, preserve, organize, search, inspect and reuse graphical resources without treating the filesystem hierarchy as the primary organization model.

Pigoune is not:

- a generic file manager;
- a general-purpose image viewer;
- an image editor;
- a collaborative digital asset management platform;
- a cloud service;
- a synchronization service.

An imported asset becomes an autonomous object of the library.

## 2. Platform

Primary target:

- Linux
- GNOME
- GTK 4
- Libadwaita
- Flatpak

The reference desktop generation is GNOME 50.

Pigoune intentionally targets Linux/GNOME rather than pursuing a cross-platform UI.

## 3. Privacy and connectivity

Pigoune is designed to work entirely offline.

It has no requirement for:

- user accounts;
- cloud services;
- telemetry;
- analytics;
- remote AI services;
- automatic uploads;
- automatic synchronization.

A URL stored as asset metadata is data only. Pigoune does not contact it automatically.

## 4. Asset ownership

Importing a file copies its content into the library.

After a successful import, the external source file and the library asset have no relationship.

Moving, renaming, modifying or deleting the external source must not affect the asset stored by Pigoune.

The source path is not part of the asset identity.

## 5. Binary preservation

Imported binary content is preserved bit-for-bit.

Pigoune may generate derived data such as:

- thumbnails;
- previews;
- search indexes;
- dominant colors;
- extracted metadata.

Derived data never replaces or modifies the stored binary object.

Pigoune is not an image editor.

## 6. Library portability

A Pigoune library is a self-contained directory.

Essential library data lives inside it, including:

- metadata database;
- stored asset objects;
- collections;
- tags;
- families;
- notes;
- recovery metadata;
- library-specific configuration.

Disposable data such as thumbnails and previews lives outside the library and can be rebuilt.

Copying the library directory must be sufficient to move or back up the library.

## 7. Reliability

The priority order is:

1. integrity;
2. recoverability;
3. predictability;
4. performance.

Performance optimizations must not weaken library safety.

## 8. Asset identity and storage

A logical asset has its own UUID.

Physical binary content is identified independently by SHA-256.

Multiple logical assets may reference the same immutable physical object when their contents are identical.

This provides transparent physical deduplication while preserving independent metadata.

## 9. Organization

An asset belongs to zero or one collection.

Assets without a collection appear under `Uncategorized`.

Collections are hierarchical.

Tags are flat and may be combined for filtering.

Tags can be renamed, merged and removed globally.

## 10. Families and variants

Several files representing variants of the same logical graphical resource may be grouped into a family.

A family owns shared metadata such as:

- display name;
- collection;
- tags;
- favorite state;
- notes;
- source and licensing information.

Each variant retains its own physical file properties.

One variant is designated as the primary representation.

## 11. Main interface

The primary desktop layout consists of:

- a navigation sidebar;
- a central asset view;
- a collapsible inspector.

The asset view supports:

- grid mode;
- list mode.

The sidebar and inspector can be collapsed independently.

The layout adapts to narrower windows.

## 12. Asset interaction

Primary interactions include:

- single-click selection;
- double-click detail view;
- Space for quick preview;
- keyboard navigation;
- multi-selection;
- drag and drop;
- clipboard import and export.

Grid thumbnails use uniform cells and never crop the asset preview.

## 13. Search

Search is local.

It supports:

- partial matching;
- case-insensitive matching;
- accent-insensitive matching;
- multiple terms;
- light typo tolerance;
- relevance ranking.

Search can operate in the current collection or across the whole library.

Filters include relevant metadata such as:

- tags;
- format;
- favorites;
- dominant color;
- orientation;
- dimensions.

Saved searches are dynamic views and are distinct from collections.

## 14. Import

An incoming file is validated before becoming an asset.

The basic import pipeline is:

1. temporary ingestion;
2. SHA-256 calculation;
3. format validation;
4. secure decoding;
5. essential metadata extraction;
6. asset creation.

Invalid files do not become library assets.

Large imports run in the background and remain cancellable.

## 15. Supported formats

Primary formats include:

- PNG
- JPEG
- SVG
- ICO
- ICNS
- WebP
- AVIF

Additional graphical formats may be accepted when supported by the decoding stack.

The library stores graphical assets only.

## 16. Deletion and undo

Deletion first moves assets to an internal trash.

Restoring an asset restores its organization and metadata.

The application supports session-level undo and redo for relevant library operations.

## 17. Export and external editing

Export returns the preserved asset content.

Opening an asset in an external editor uses a working copy rather than exposing the internal immutable object for modification.

Pigoune does not register itself as the system's general image viewer.

## 18. Integrity and recovery

Stored objects are associated with their SHA-256 identity.

Pigoune provides integrity verification and recovery mechanisms.

SQLite is the metadata source of truth.

A separate recovery manifest provides an additional path for rebuilding critical library metadata if necessary.

Crash recovery must return the library to its last coherent state.

## 19. Technical architecture

The application is implemented in Rust.

The domain/library core remains independent from GTK.

The main technologies are expected to include:

- Rust;
- GTK 4;
- Libadwaita;
- SQLite;
- FTS5;
- Glycin;
- Flatpak.

SQLite is used with durability-first settings on local storage.

Search is abstracted so its implementation can evolve independently from the rest of the core.

Background work is coordinated by a central job manager.

## 20. Version 1.0 scope

Version 1.0 should provide a complete, reliable daily-use experience around:

- autonomous libraries;
- content-addressed storage;
- physical deduplication;
- robust imports;
- core graphical formats;
- grid and list views;
- hierarchical collections;
- tags;
- favorites;
- local search and filters;
- saved searches;
- inspector;
- display names;
- notes and source/license metadata;
- variant families;
- quick preview;
- detail view;
- drag and drop;
- clipboard workflows;
- trash;
- undo/redo;
- content replacement;
- export;
- backup and restore;
- integrity checking;
- crash recovery;
- recovery metadata;
- adaptive GNOME UI;
- keyboard navigation;
- command palette;
- Flatpak distribution;
- French and English localization.

Quality takes priority over feature count.

## 21. Post-1.0 directions

The architecture should leave room for later work such as:

- visual similarity detection;
- advanced comparison mode;
- full NAS/network-library support;
- multiple libraries;
- maintenance CLI;
- stable/beta release channels.

These features must not delay a high-quality 1.0.

## 22. UX principles

- Prefer reversible actions over unnecessary confirmation dialogs.
- Never perform destructive operations silently.
- Do not expose unfinished or placeholder features.
- Keep the interface calm with large libraries.
- Keep graphical assets visually central.
- Do not turn Pigoune into a file manager with larger thumbnails.
- Heavy operations must not freeze the UI.
- Persisted actions must actually be durable.
- Protect data integrity before optimizing performance.
- Keep implementation complexity invisible during normal use.
