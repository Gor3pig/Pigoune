# Icon containers

An imported icon container is one Pigoune asset. Its internal images are `ContainerRepresentation` values: they describe container representations and are not family variants. The published object preserves the original container bytes.

## ICO inventory

The structural parser in `pigoune-core` reads only the internal copy in `objects/.tmp`. It inventories every ICO entry before publication. Every recognized entry must be structurally valid; one invalid entry rejects the entire import. Identical payload intervals are accepted as two distinct ordinal representations, while partial overlaps are rejected. CUR is not accepted as ICO.

PNG payloads are checked by signature and chunk structure, including CRCs and IHDR dimensions. Supported DIB payloads use `BITMAPINFOHEADER` (40 bytes), `BI_RGB`, 1, 4, 8, 24, or 32 bit depth, complete XOR rows, and a complete AND mask. Their header height includes XOR and AND: the representation height is half that value. Other DIB header variants, compressions, and bit depths are rejected explicitly.

Limits shared by icon containers are 1,024 representations, 4,096 px per dimension, and 64 MiB for the theoretical `width × height × 4` budget of one representation. The ICO parser applies these same limits. Sizes, offsets, and row calculations use checked arithmetic. The inventory does not decode pixels.

## Primary representation

For ICO, the primary is selected stably in this order: descending area, descending largest dimension, PNG before DIB at equal dimensions, descending known encoded bit depth, then ascending ordinal. The `scale` field is `None` for ICO.

Only this primary must be decoded during import. For PNG, Pigoune passes the selected payload directly to the Glycin PNG loader through a bounded slice of staging. For DIB, Pigoune gives Glycin a synthetic single-entry ICO view: a 22-byte in-memory header followed by the same bounded reader. Decoded dimensions must match the inventory. The view is never published; the stored object remains the original ICO, bit for bit.

## ICNS 1.0 inventory

Pigoune supports modern ICNS representations and the main explicitly documented legacy representations. Its parser in `pigoune-core` is the sole structural authority for the container. It reads staging, bounds every element, and inventories representations before publication. The TOC, `icnV`, and other recognized metadata elements remain in the original object without becoming representations; the TOC is never an authoritative index. Truly unknown FourCCs are bounded, preserved, and reported through an aggregated, non-persisted warning. Known historical monochrome and palette types are explicitly rejected in 1.0.

The modern types `icp4`, `ic11`, `icp5`, `ic12`, `icp6`, `ic07`, `ic13`, `ic08`, `ic14`, `ic09`, and `ic10` accept PNG, JP2, or a recognized J2K codestream. `icsB`, `sb24`, and `SB24` use the same path. `ic04`, `ic05`, and `icsb` also accept their actual ARGB/RLE encoding. The RGB/RLE pairs `is32+s8mk`, `il32+l8mk`, `ih32+h8mk`, and `it32+t8mk` each require exactly one color element and one 8-bit alpha mask. PNG payloads use the chunk and CRC checker shared with ICO. JPEG 2000 payloads are bounded and recognized by their signature and initial structure; Glycin confirms the primary's decoded pixels. Secondary JPEG 2000 coefficients are not exhaustively validated.

PNG, JP2, and J2K go directly to Glycin through a bounded staging slice. RGB/RLE and ARGB/RLE primaries use only the targeted decoders in `icns` 0.5.0, without its PNG/JP2 features and without `IconFamily::read`. Decoded dimensions must match the FourCC. Inventory dimensions are physical and `scale` is 1 or 2; the ordinal is the graphical element's physical position among all ICNS elements, so gaps are possible.

The ICNS primary is selected by descending physical area, descending largest dimension, scale 1 before 2 at equal pixels, codec preference PNG before JPEG 2000 before ARGB before RGB, descending known bit depth, then ascending physical ordinal. ICNS limits are 2,048 elements, 64 MiB encoded per element, 256 MiB per file, and 256 MiB cumulative theoretical decoded size. The shared limits of 1,024 representations, 4,096 px per dimension, and 64 MiB decoded per representation also apply.

The ICO or ICNS inventory is available in validation and import results for the current session. It is not persisted yet. The SQLite schema remains version 2 and the library format remains version 1.
