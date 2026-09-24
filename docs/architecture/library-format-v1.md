# Format de bibliothèque v1

La version de l'application (Cargo), la version du format de bibliothèque (`LIBRARY_FORMAT_VERSION = 1`) et celle du schéma SQLite (`DATABASE_SCHEMA_VERSION = 3`) évoluent indépendamment. `library.json` porte la version du format ; `PRAGMA user_version` porte celle du schéma. Une migration SQL ne change pas automatiquement le format global.

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

## Création

La destination finale doit être inexistante. Pigoune construit la bibliothèque complète dans un dossier temporaire frère, sur le même système de fichiers, puis la publie par renommage atomique `RENAME_NOREPLACE` sans remplacer une destination apparue entre-temps. `library.json` est complet et synchronisé avant la publication. Une erreur avant celle-ci ne laisse aucune bibliothèque partielle au chemin final. Un crash peut laisser un dossier frère `.pigoune-create-*` ; une autre création ne supprime jamais automatiquement ces restes.

## Identité et métadonnées

`library.json` contient uniquement `type: "pigoune-library"`, `library_id` (UUID v4 canonique, minuscule avec tirets) et `format_version: 1`. La table `library_metadata` contient le même `LibraryId` en BLOB de 16 octets dans une unique ligne `singleton = 1`. L'ouverture en écriture compare ces identités et échoue si elles divergent. Un manifest invalide n'est jamais réparé silencieusement.

La migration SQL `0001_initial.sql` crée `library_metadata`, `objects`, `assets` et l'index `assets_object_hash_idx`. La migration `0002_image_metadata.sql` reconstruit `objects` et `assets` dans une transaction afin de préserver leur clé étrangère et d'imposer la complétude des métadonnées. La migration `0003_container_representations.sql` ajoute `object_representations` et son index partiel unique sur la principale sans reconstruire `objects` ni `assets`. Les tables utilisent `STRICT` ; SQLite 3.37 est la version minimale. Pigoune utilise `rusqlite` lié à SQLite système, sans SQLite embarqué. Le SDK/runtime GNOME 50 est la référence.

Le schéma SQLite v2 conserve sur `objects` les propriétés intrinsèques validées : `format` (code canonique indépendant du MIME et de l'extension), `width`, `height` et `animated` (0 ou 1). Une contrainte SQL impose que les quatre champs soient soit tous `NULL`, soit tous renseignés avec un format non vide et des dimensions strictement positives. Les objets issus du schéma v1 ont des métadonnées inconnues (`NULL`) ; Pigoune ne les déduit jamais du chemin ni du nom. Une validation ultérieure du même contenu peut les renseigner atomiquement. Une version de Pigoune qui ne connaît pas un code de format stocké renvoie actuellement une erreur explicite à la lecture. Les métadonnées intrinsèques validées devront aussi être représentables dans un futur manifest de récupération.

Le schéma SQLite v3 conserve les inventaires ICO/ICNS validés sur l'objet physique dans `object_representations` : ordinal physique, dimensions, échelle et profondeur optionnelles, code stable du codec, taille encodée et indicateur de principale. La clé `(object_hash, ordinal)` préserve les trous ; un index partiel unique autorise au plus une principale par objet, tandis que le lecteur en exige exactement une si des lignes existent. Les anciens objets ICO/ICNS issus des schémas v1/v2 peuvent ne pas avoir d'inventaire ; la migration ne les reparcourt pas et n'en invente aucun. Un import validé ultérieur les enrichit atomiquement avec son asset. Un inventaire existant doit correspondre exactement, sinon l'import échoue. Décalages des payloads et masques, références de masque, FourCC, éléments ICNS inconnus, détails du parseur, données décodées et avertissements d'import ne sont pas conservés. Une opération future demandant un payload précis doit reparcourir et revalider l'objet immuable. Le futur manifest de récupération devra également représenter le `ContainerMetadata` persisté.

Les UUID d'assets sont des BLOB de 16 octets ; les SHA-256 d'objets, des BLOB de 32 octets. `objects` conserve la taille et le chemin relatif sous `objects/`. `assets` référence un objet par clé étrangère, conserve le nom de fichier d'origine comme octets POSIX exacts dans un BLOB, un nom d'affichage UTF-8 dans un TEXT et `imported_at_utc_us` comme entier signé non négatif représentant un instant UTC en microsecondes depuis l'époque Unix. Aucun chemin externe source n'est conservé.

Chaque connexion d'écriture active les clés étrangères, un délai d'attente de cinq secondes et `synchronous = FULL`, puis vérifie `journal_mode = WAL`. Les migrations SQL embarquées sont montantes, séquentielles et transactionnelles ; `user_version` est mis à jour dans la transaction correspondante. Une base d'une version plus récente est refusée en écriture.

Les nouveaux octets sont d'abord copiés dans `objects/.tmp` et validés à partir de cette copie. `ObjectStore` publie ensuite durablement l'objet physique, avant la transaction SQLite qui enregistre ou réconcilie `objects` et `object_representations`, crée `assets`, puis valide. En cas d'échec de SQLite, la transaction ne crée pas d'asset partiel ; le fichier physique peut rester orphelin jusqu'à une maintenance sûre. La clé étrangère empêche un asset de référencer un objet absent de la base. Une vérification complète des octets est une opération d'intégrité distincte des consultations ordinaires.
