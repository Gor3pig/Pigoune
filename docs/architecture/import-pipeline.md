# Pipeline d'import normal

Le service synchrone `ImportService::import_file`, dans `pigoune-app`, s'exécute dans un worker, jamais sur le thread principal GTK. `LibraryDatabase` sert actuellement de writer sérialisé. La future UI appelle ce service avec une bibliothèque, un fichier source, un nom d'affichage et une politique de doublon.

Le pipeline suit cet ordre : validation des arguments et extraction des octets POSIX du seul nom de fichier → copie dans `objects/.tmp` et SHA-256 → validation Glycin, analyse SVG ou inventaire structurel ICO depuis le staging → recherche des assets logiques par hash en mode `Detect` → publication durable de l'objet → horodatage UTC et nouvel `AssetId` → transaction SQLite. Pour ICO, Glycin décode exactement la représentation principale choisie par l'inventaire, via une vue mono-entrée. Après le staging, le fichier source n'est plus lu ; aucun chemin source n'est enregistré. Les octets publiés restent identiques aux octets copiés.

Un **doublon logique** existe si au moins un `AssetId` référence déjà le même `ObjectHash` dans SQLite. `Detect` retourne ces identifiants et les avertissements de validation sans publier ni créer d'asset. `ImportAnyway` crée un asset distinct. La **déduplication physique** réutilise l'objet immuable de même SHA-256, y compris s'il est orphelin et ne constitue donc pas un doublon logique.

Les avertissements de validation accompagnent un résultat `Imported` ou `Duplicate` et ne sont pas persistés. Si la transaction SQLite échoue après publication, elle ne laisse aucun asset partiel ; l'objet physique peut rester orphelin. Le service ne le supprime pas automatiquement, car il peut être partagé ou avoir été publié concurremment. Sa suppression sûre relève de la maintenance.
