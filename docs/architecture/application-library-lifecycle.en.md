# Application library lifecycle

Pigoune V1 uses one primary library. The managed location is calculated on every launch as `user_data_dir()/APPLICATION_ID/library`. The application may create its parent; only `Library::create` creates the final destination.

The `user_config_dir()/APPLICATION_ID/config.json` file stores a typed locator: `managed-default` without a path, or `file-uri` with the URI of the `gio::File` granted by the picker. The JSON format has `type: "pigoune-app-config"` and `version: 1`. Loading rejects unknown types, versions, and locators. An external URI must be `file:` and provide a local path through GIO. No assumed host path is reconstructed.

Saving creates the configuration directory when needed and syncs its parent directories, writes a sibling temporary file, syncs its contents, atomically replaces `config.json`, then syncs its directory. Configuration is written only after creation completes and reopens, or after a successful open. If persistence fails, the session stays open with a warning and a retry action; any previous configuration is not removed beforehand. If synchronization fails after replacement, the application reports that the change's durability could not be confirmed.

One worker owns the `LibrarySession`, its `Library`, locator, and identity. The UI receives only `Welcome`, `Opening`, `Open`, and `OpenError` states through a `MainContext` source. On startup, absent configuration shows Welcome, valid configuration attempts an open, and failure never creates a library automatically. `Forget this library location` removes only `config.json`, syncs its parent, and returns to Welcome after confirmation; it never removes a library. If synchronization fails after removal, the UI reports uncertain durability and offers a retry. That retry syncs the directory even when `config.json` is already absent.

External libraries are chosen through `GtkFileDialog`. A creation parent is selected before GIO forms the child folder. The Flatpak portal grants selected access without additional broad permission. URI persistence, opening after relaunch, SQLite WAL, ObjectStore, and atomic publication must be checked in a real GNOME session; unit tests do not establish those guarantees.

A GNOME 50 test on a portal-granted folder returned a SQLite I/O error during creation, with a FUSE document path; the final destination was not published. This result does not establish external storage support through that portal. It does not justify broad Flatpak permission or weaker SQLite and publication guarantees.

The application ID separates Devel and Stable XDG spaces. The autonomous library format remains compatible across editions when their versions permit it.
