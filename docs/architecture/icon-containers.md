# Conteneurs d'icônes

Un conteneur d'icônes importé constitue un seul asset Pigoune. Ses images internes sont des `ContainerRepresentation` : elles décrivent les représentations du conteneur et ne sont pas des variantes de famille. L'objet publié conserve les octets originaux du conteneur.

## Inventaire ICO

Le parseur structurel de `pigoune-core` lit uniquement la copie interne dans `objects/.tmp`. Il inventorie toutes les entrées de l'ICO avant publication. Chaque entrée reconnue doit être structurellement valide ; une entrée invalide fait échouer l'import entier. Les intervalles de payload identiques sont acceptés comme deux représentations ordinales distinctes, tandis que les chevauchements partiels sont rejetés. CUR n'est pas accepté comme ICO.

Les payloads PNG sont vérifiés par leur signature et leur structure de chunks, y compris les CRC et les dimensions IHDR. Les DIB pris en charge utilisent `BITMAPINFOHEADER` (40 octets), `BI_RGB`, les profondeurs 1, 4, 8, 24 ou 32 bits, des lignes XOR et un masque AND complets. Leur hauteur d'en-tête inclut XOR et AND : la hauteur de la représentation est la moitié de cette valeur. Les autres variantes d'en-tête, compressions et profondeurs DIB sont rejetées explicitement.

Les limites communes aux conteneurs d'icônes sont 1 024 représentations, 4 096 px par dimension et 64 Mio pour le budget théorique `width × height × 4` d'une représentation. Le parseur ICO applique ces mêmes limites. Les tailles, décalages et calculs de lignes utilisent une arithmétique contrôlée. L'inventaire ne décode aucun pixel.

## Représentation principale

Pour ICO, la principale est choisie de manière stable, dans cet ordre : aire décroissante, plus grande dimension décroissante, PNG avant DIB à dimensions égales, profondeur encodée connue décroissante, puis ordinal croissant. Le champ `scale` vaut `None` pour ICO.

Seule cette principale doit être décodée pendant l'import. Pour PNG, Pigoune transmet directement le payload sélectionné au loader PNG Glycin par une sous-plage bornée du staging. Pour DIB, Pigoune fournit à Glycin une vue ICO synthétique mono-entrée : un en-tête de 22 octets en mémoire suivi de cette même lecture bornée. Les dimensions décodées doivent correspondre à l'inventaire. La vue n'est jamais publiée ; l'objet stocké reste l'ICO original, bit pour bit.

## Inventaire ICNS 1.0

Pigoune prend en charge les représentations ICNS modernes ainsi que les principales représentations legacy explicitement documentées. Son parseur dans `pigoune-core` est l'unique autorité structurelle du conteneur. Il lit le staging, borne chaque élément et inventorie ses représentations avant toute publication. Le TOC, `icnV` et les autres éléments de métadonnées reconnus restent dans l'objet original, sans devenir des représentations ; le TOC n'est jamais un index d'autorité. Les FourCC réellement inconnus sont bornés, préservés et signalés par un avertissement agrégé non persisté. Les types monochromes et palettes historiques connus sont rejetés explicitement en 1.0.

Les types modernes `icp4`, `ic11`, `icp5`, `ic12`, `icp6`, `ic07`, `ic13`, `ic08`, `ic14`, `ic09` et `ic10` acceptent PNG, JP2 ou un codestream J2K reconnu. `icsB`, `sb24` et `SB24` suivent ce même chemin. `ic04`, `ic05` et `icsb` acceptent aussi leur encodage ARGB/RLE réel. Les paires RGB/RLE `is32+s8mk`, `il32+l8mk`, `ih32+h8mk` et `it32+t8mk` exigent chacune une couleur et un masque alpha 8 bits uniques. Les payloads PNG sont vérifiés par le contrôleur de chunks et CRC partagé avec ICO. Les payloads JPEG 2000 sont bornés et reconnus par leur signature et leur structure initiale ; le décodeur Glycin confirme intégralement les pixels de la principale. Les secondaires JPEG 2000 ne subissent pas une validation exhaustive des coefficients.

PNG, JP2 et J2K sont transmis directement à Glycin par une sous-plage bornée du staging. Les principales RGB/RLE et ARGB/RLE passent uniquement par les décodeurs ciblés de `icns` 0.5.0, sans ses features PNG/JP2 et sans `IconFamily::read`. Les dimensions décodées doivent correspondre au FourCC. Les dimensions de l'inventaire sont physiques et `scale` vaut 1 ou 2 ; l'ordinal désigne la position physique de l'élément graphique parmi tous les éléments ICNS, avec des trous possibles.

La principale ICNS est choisie par aire physique décroissante, plus grande dimension décroissante, échelle 1 avant 2 à pixels égaux, codec PNG avant JPEG 2000 avant ARGB avant RGB, profondeur connue décroissante, puis ordinal physique croissant. Les limites ICNS sont 2 048 éléments, 64 Mio encodés par élément, 256 Mio par fichier et 256 Mio de budget théorique décodé cumulé. Les limites communes de 1 024 représentations, 4 096 px par dimension et 64 Mio décodés par représentation s'appliquent aussi.

L'inventaire ICO ou ICNS reste disponible dans les résultats de validation et d'import et est conservé sur l'objet physique par le schéma SQLite v3 ; le format de bibliothèque reste en version 1. `encoded_size` compte tous les octets de payload encodé nécessaires à une représentation logique : payload PNG ou DIB complet pour ICO ; payload de l'élément graphique pour ICNS autonome ; payloads couleur et masque 8 bits requis pour une paire ICNS RGB legacy. Le masque n'a pas de représentation séparée, et l'ordinal reste celui de l'élément couleur physique. La base ne conserve ni décalages de payload, ni références de masque, ni FourCC, ni éléments inconnus, ni détails de parseur, ni données décodées, ni avertissements. L'accès à un payload précis exige de reparcourir et revalider l'objet immuable.
