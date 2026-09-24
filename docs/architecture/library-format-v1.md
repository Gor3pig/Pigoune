# Format de bibliothèque v1

La version de l'application (Cargo), la version du format de bibliothèque (`LIBRARY_FORMAT_VERSION = 1`) et celle du schéma SQLite (`DATABASE_SCHEMA_VERSION = 1`) évoluent indépendamment. `library.json` porte la version du format ; `PRAGMA user_version` porte celle du schéma. Une migration SQL ne change pas automatiquement le format global.

## Dossier

```text
<library-root>/
├── library.json
├── library.db
├── objects/
│   ├── .tmp/
│   ├── .lock
│   └── <shards...>
└── recovery/
```

Les caches sont hors de ce dossier. En mode WAL, `library.db-wal` et `library.db-shm` peuvent apparaître à sa racine ; une copie ou sauvegarde doit représenter un état SQLite cohérent.

## Identité et métadonnées

`library.json` contient uniquement `type: "pigoune-library"`, `library_id` (UUID v4 canonique, minuscule avec tirets) et `format_version: 1`. La table `library_metadata` contient le même `LibraryId` en BLOB de 16 octets dans une unique ligne `singleton = 1`. L'ouverture en écriture compare ces identités et échoue si elles divergent. Un manifest invalide n'est jamais réparé silencieusement.

La migration SQL `0001_initial.sql` crée seulement `library_metadata`, `objects`, `assets` et l'index `assets_object_hash_idx`. Les tables utilisent `STRICT` ; SQLite 3.37 est la version minimale. Pigoune utilise `rusqlite` lié à SQLite système, sans SQLite embarqué. Le SDK/runtime GNOME 50 est la référence.

Les UUID d'assets sont des BLOB de 16 octets ; les SHA-256 d'objets, des BLOB de 32 octets. `objects` conserve la taille et le chemin relatif sous `objects/`. `assets` référence un objet par clé étrangère, conserve le nom de fichier d'origine comme octets POSIX exacts dans un BLOB, un nom d'affichage UTF-8 dans un TEXT et `imported_at_utc_us` comme entier signé non négatif représentant un instant UTC en microsecondes depuis l'époque Unix. Aucun chemin externe source n'est conservé.

Chaque connexion d'écriture active les clés étrangères, un délai d'attente de cinq secondes et `synchronous = FULL`, puis vérifie `journal_mode = WAL`. Les migrations SQL embarquées sont montantes, séquentielles et transactionnelles ; `user_version` est mis à jour dans la transaction correspondante. Une base d'une version plus récente est refusée en écriture.

L'objet physique est publié durablement par `ObjectStore` avant la transaction SQLite qui enregistre ou réconcilie `objects`, crée `assets`, puis valide. En cas d'échec de SQLite, la transaction ne crée pas d'asset partiel ; le fichier physique peut rester orphelin jusqu'à une maintenance sûre. La clé étrangère empêche un asset de référencer un objet absent de la base. Une vérification complète des octets est une opération d'intégrité distincte des consultations ordinaires.
