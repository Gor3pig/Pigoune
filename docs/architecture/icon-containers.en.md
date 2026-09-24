# Icon containers

An imported icon container is one Pigoune asset. Its internal images are `ContainerRepresentation` values: they describe container representations and are not family variants. The published object preserves the original container bytes.

## ICO inventory

The structural parser in `pigoune-core` reads only the internal copy in `objects/.tmp`. It inventories every ICO entry before publication. Every recognized entry must be structurally valid; one invalid entry rejects the entire import. Identical payload intervals are accepted as two distinct ordinal representations, while partial overlaps are rejected. CUR is not accepted as ICO.

PNG payloads are checked by signature and chunk structure, including CRCs and IHDR dimensions. Supported DIB payloads use `BITMAPINFOHEADER` (40 bytes), `BI_RGB`, 1, 4, 8, 24, or 32 bit depth, complete XOR rows, and a complete AND mask. Their header height includes XOR and AND: the representation height is half that value. Other DIB header variants, compressions, and bit depths are rejected explicitly.

Limits shared by icon containers are 1,024 representations, 4,096 px per dimension, and 64 MiB for the theoretical `width × height × 4` budget of one representation. The ICO parser applies these same limits. Sizes, offsets, and row calculations use checked arithmetic. The inventory does not decode pixels.

## Primary representation

The primary is selected stably in this order: descending area, descending largest dimension, PNG before DIB at equal dimensions, descending known encoded bit depth, then ascending ordinal. The generic model's `scale` field is reserved for a possible ICNS representation scale; it is `None` for ICO.

Only this primary must be decoded during import. For PNG, Pigoune passes the selected payload directly to the Glycin PNG loader through a bounded slice of staging. For DIB, Pigoune gives Glycin a synthetic single-entry ICO view: a 22-byte in-memory header followed by the same bounded reader. Decoded dimensions must match the inventory. The view is never published; the stored object remains the original ICO, bit for bit.

The inventory is available in the validation and import results for the current session. It is not persisted yet. The SQLite schema remains version 2 and the library format remains version 1. The persistence model will be finalized after the ICNS step.
