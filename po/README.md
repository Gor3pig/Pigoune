# Traductions de Pigoune

L’interface utilise les chaînes anglaises du code comme langue source et le domaine gettext `pigoune`. Le catalogue français est obligatoire ; aucun catalogue anglais n’est nécessaire.

Depuis la racine du dépôt, après avoir installé `xtr` et GNU gettext :

```sh
xtr --package-name=Pigoune --package-version=0.1.0 \
  --copyright-holder='Pigoune contributors' \
  --msgid-bugs-address=https://github.com/Gor3pig/Pigoune/issues \
  --output=po/pigoune.pot crates/pigoune-app/src/main.rs
msgmerge --update --backup=none po/fr.po po/pigoune.pot
msgfmt --check --statistics --output-file=/dev/null po/fr.po
```

`xtr` suit les modules Rust depuis l’entrée indiquée dans `POTFILES.in`. Chaque nouvelle chaîne visible doit être marquée par `gettext("English source text")`, puis traduite dans `fr.po`. Les noms provenant des données de l’utilisateur ne sont jamais traduits automatiquement.
