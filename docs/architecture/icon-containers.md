# Conteneurs d'icônes

Un conteneur d'icônes importé constitue un seul asset Pigoune. Ses images internes sont des `ContainerRepresentation` : elles décrivent les représentations du conteneur et ne sont pas des variantes de famille. L'objet publié conserve les octets originaux du conteneur.

## Inventaire ICO

Le parseur structurel de `pigoune-core` lit uniquement la copie interne dans `objects/.tmp`. Il inventorie toutes les entrées de l'ICO avant publication. Chaque entrée reconnue doit être structurellement valide ; une entrée invalide fait échouer l'import entier. Les intervalles de payload identiques sont acceptés comme deux représentations ordinales distinctes, tandis que les chevauchements partiels sont rejetés. CUR n'est pas accepté comme ICO.

Les payloads PNG sont vérifiés par leur signature et leur structure de chunks, y compris les CRC et les dimensions IHDR. Les DIB pris en charge utilisent `BITMAPINFOHEADER` (40 octets), `BI_RGB`, les profondeurs 1, 4, 8, 24 ou 32 bits, des lignes XOR et un masque AND complets. Leur hauteur d'en-tête inclut XOR et AND : la hauteur de la représentation est la moitié de cette valeur. Les autres variantes d'en-tête, compressions et profondeurs DIB sont rejetées explicitement.

Les limites communes aux conteneurs d'icônes sont 1 024 représentations, 4 096 px par dimension et 64 Mio pour le budget théorique `width × height × 4` d'une représentation. Le parseur ICO applique ces mêmes limites. Les tailles, décalages et calculs de lignes utilisent une arithmétique contrôlée. L'inventaire ne décode aucun pixel.

## Représentation principale

La principale est choisie de manière stable, dans cet ordre : aire décroissante, plus grande dimension décroissante, PNG avant DIB à dimensions égales, profondeur encodée connue décroissante, puis ordinal croissant. Le champ `scale` du modèle générique est réservé à une éventuelle échelle de représentation ICNS ; il vaut `None` pour ICO.

Seule cette principale doit être décodée pendant l'import. Pour PNG, Pigoune transmet directement le payload sélectionné au loader PNG Glycin par une sous-plage bornée du staging. Pour DIB, Pigoune fournit à Glycin une vue ICO synthétique mono-entrée : un en-tête de 22 octets en mémoire suivi de cette même lecture bornée. Les dimensions décodées doivent correspondre à l'inventaire. La vue n'est jamais publiée ; l'objet stocké reste l'ICO original, bit pour bit.

L'inventaire est disponible dans le résultat de validation et dans le résultat d'import pour la session en cours. Il n'est pas encore persisté. Le schéma SQLite reste en version 2 et le format de bibliothèque en version 1. Le modèle de persistance sera fixé après l'étape ICNS.
