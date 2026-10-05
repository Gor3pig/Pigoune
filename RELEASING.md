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
   release. Write the release notes there, following the layout below, then publish it.
5. Publishing the release starts the `Website` workflow, which puts the package in the signed
   Flatpak repository at `https://gor3pig.github.io/Pigoune/repo/`. Installed copies of
   Pigoune then offer the update. This run starts from the release tag, which is why the
   `github-pages` environment accepts deployments from `main` and from `v*` tags. GitHub Pages
   ignores a deployment whose build version it has already seen, and the push of the release
   commit to `main` may already have published the website from the same commit, so each
   deployment gets a build version made of the commit and the run number.

## The Flatpak repository

The repository only holds the latest release. `website/flatpak-repo.sh` imports the package
of that release into a new repository and signs it with the key stored in the
`FLATPAK_REPO_SIGNING_KEY` secret. Its public part is `website/pigoune.gpg`, also embedded in
`pigoune.flatpakref` and `pigoune.flatpakrepo`. The maintainer keeps an offline backup of the
private key; if it is ever lost or stolen, a new key must be created and users must install
Pigoune again from the new `.flatpakref` file.

## Release notes layout

The notes of a GitHub release are written in English first, then in French, with the same
content in both languages:

1. A short summary of the release.
2. The changes, grouped under headings such as *New*, *Improved* and *Fixed*.
3. **Install**: how to set up Flatpak if needed, then install Pigoune from
   `https://gor3pig.github.io/Pigoune/pigoune.flatpakref` with Software or with
   `flatpak install --user`. The attached `.flatpak` file remains an alternative.
4. **Update**: installations from the repository update themselves. Remind people who
   installed 1.5 or older from a `.flatpak` file to switch once: uninstall, then install from
   the `.flatpakref` file, keeping libraries and settings.
5. A link to the user guide.

## Rebuilding a package

The release workflow can also be started by hand from the Actions tab, with an existing tag,
to rebuild and attach the Flatpak package of that release again.
