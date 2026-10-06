# Translating Pigoune

Pigoune uses gettext. The source language is American English, and each translation lives in
a `<language>.po` file in this folder. Thank you for helping!

## Updating an existing translation

1. Refresh the template and the existing translations:

   ```sh
   meson setup _build
   meson compile -C _build pigoune-update-po
   ```

2. Open `po/<language>.po` in a translation editor such as
   [Translation Editor (Gtranslator)](https://flathub.org/apps/org.gnome.Gtranslator) or
   Poedit, and translate every empty or "fuzzy" entry.

3. Check your work:

   ```sh
   ./check.sh
   ```

## What to translate

French is translated completely: the interface, the application description shown by
software centers, the launcher and the settings descriptions. The other languages translate
**only the interface**, the entries that come from `crates/`. Leave the entries that come from
`data/` empty: `./check.sh` refuses them for any language other than French.

## Adding a new language

1. Create the file from the template, replacing `de` with your language code:

   ```sh
   meson setup _build
   meson compile -C _build pigoune-pot
   msginit --locale=de --input=po/pigoune.pot --output-file=po/de.po
   ```

2. Add the language code to `po/LINGUAS`, keeping the list in alphabetical order.

3. Translate the file, then run `./check.sh`.

The new language is then offered in **Preferences › General › Interface Language**, and
Pigoune uses it on its own for people whose system is in that language. If the language list
shows a code instead of the name of your language, add its name, written in your language,
to `NATIVE_NAMES` in `crates/pigoune-app/src/languages.rs`.

## Tips

- Keep placeholders such as `{name}` or `{count}` exactly as they are; only move them where
  your language needs them.
- Keep the mnemonic underscore (`_Open`) on a letter of the translated word.
- Follow the typography of your language (quotes, spaces before punctuation, ...), but use
  plain hyphens `-` rather than long dashes.
- To try your translation, choose your language in **Preferences › General › Interface
  Language** and restart Pigoune, or run it with `LANGUAGE=de cargo run -p pigoune-app`.
