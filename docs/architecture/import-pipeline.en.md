# Normal import pipeline

The synchronous `ImportService::import_file` in `pigoune-app` runs on a worker, never on the GTK main thread. `LibraryDatabase` currently acts as a serialized writer. The future UI calls this service with a library, a source file, a display name, and a duplicate policy.

The pipeline follows this order: validate arguments and extract the POSIX bytes of the filename alone → copy into `objects/.tmp` and calculate SHA-256 → validate with Glycin, analyze SVG, or inventory ICO/ICNS structure from staging → look up logical assets by hash in `Detect` mode → durably publish the object → set the UTC timestamp and create a new `AssetId` → SQLite transaction. For ICO, Glycin decodes the primary: a direct PNG payload or a single-entry DIB view. For ICNS, the PNG/JP2/J2K primary goes directly to Glycin; a legacy primary goes to the targeted `icns` decoder after structural validation. The source file is not read after staging; no source path is stored. Published bytes remain identical to the copied bytes.

A **logical duplicate** exists when at least one `AssetId` already references the same `ObjectHash` in SQLite. `Detect` returns those IDs and validation warnings without publishing or creating an asset. `ImportAnyway` creates a distinct asset. **Physical deduplication** reuses the immutable object with the same SHA-256, including when it is orphaned and therefore is not a logical duplicate.

Validation warnings accompany either an `Imported` or `Duplicate` result and are not persisted. If the SQLite transaction fails after publication, it leaves no partial asset; the physical object may remain orphaned. The service does not delete it automatically because it may be shared or concurrently published. Safe deletion belongs to maintenance.
