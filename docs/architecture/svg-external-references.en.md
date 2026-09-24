# External SVG references

Import preserves the staged SVG bytes and publishes exactly those bytes after validation. Glycin decodes a first frame from internal staging. An independent streaming XML analysis reads the same staging again; it does not rewrite the SVG or resolve any resource, entity, or URI. Nothing is downloaded or embedded automatically.

Fragments in the current document (`#id`) and `data:` URIs are internal. Other nonempty rendering references, relative or absolute, are external. Navigation links using `<a href>` are not rendering dependencies. The analysis covers `href` and XLink attributes on resource elements, `url()` in supported presentation attributes, CSS styles, `@import`, `xml-stylesheet` instructions, and external identifiers in DOCTYPE and entity declarations.

An external reference produces the nonblocking `SvgExternalReferences` warning when Glycin has produced a valid first frame and dimensions. A structural analysis failure blocks validation: an incomplete read cannot establish that no external references exist. Warnings remain in memory; neither raw URIs nor warnings are persisted in SQLite for now.

The DTD parser does not cover every XML declaration. A declaration it omits therefore causes a validation error. The scanner does not handle compressed SVG (SVGZ) until safe decompressed reading from staging is defined.
