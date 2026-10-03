# Releasing Pigoune

This page is for maintainers. Contributors do not need it: releases are made by the
maintainer of the repository.

## Making a release

1. Update the version in `meson.build` and `Cargo.toml`, run `cargo update -w`, and add the
   release notes to `data/io.github.gor3pig.Pigoune.metainfo.xml.in`, with their French
   translation.
2. Point the screenshot URLs in the metainfo at the new tag.
3. Commit, tag the commit with `vX.Y.Z` and push the tag.
4. The release workflow builds the Flatpak package from the tag and attaches it to a draft
   release. Write the release notes there, then publish it.

## Rebuilding a package

The release workflow can also be started by hand from the Actions tab, with an existing tag,
to rebuild and attach the Flatpak package of that release again.
