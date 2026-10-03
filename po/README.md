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

## Adding a new language

1. Create the file from the template, replacing `de` with your language code:

   ```sh
   meson setup _build
   meson compile -C _build pigoune-pot
   msginit --locale=de --input=po/pigoune.pot --output-file=po/de.po
   ```

2. Add the language code to `po/LINGUAS`, keeping the list in alphabetical order.

3. Translate the file, then run `./check.sh`.

## Tips

- Keep placeholders such as `{name}` or `{count}` exactly as they are; only move them where
  your language needs them.
- Keep the mnemonic underscore (`_Open`) on a letter of the translated word.
- Follow the typography of your language (quotes, spaces before punctuation, ...), but use
  plain hyphens `-` rather than long dashes.
- To try your translation, run Pigoune in your language, for example
  `LANGUAGE=de cargo run -p pigoune-app`.
