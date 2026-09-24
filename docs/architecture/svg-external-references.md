# Références externes des SVG

L'import conserve les octets du SVG staged et publie exactement ces octets après validation. Glycin décode une première frame depuis le staging interne. Une analyse XML streaming indépendante relit ce même staging ; elle ne réécrit pas le SVG et ne résout aucune ressource, entité ni URI. Aucun téléchargement ou incorporation automatique n'a lieu.

Les fragments du document courant (`#id`) et les URI `data:` sont internes. Les autres références de rendu non vides, relatives ou absolues, sont externes. Les liens de navigation `<a href>` ne sont pas des dépendances de rendu. L'analyse couvre les attributs `href` et XLink des éléments de ressource, les `url()` des attributs de présentation pris en charge, les styles CSS, `@import`, les instructions `xml-stylesheet` et les identifiants externes des DOCTYPE et déclarations d'entités.

Une référence externe produit l'avertissement non bloquant `SvgExternalReferences` si Glycin a fourni une première frame et des dimensions valides. Un échec de l'analyse structurelle bloque la validation : l'absence de référence ne peut pas être déduite d'une lecture incomplète. Les avertissements restent en mémoire ; ni les URI brutes ni les avertissements ne sont persistés dans SQLite pour l'instant.

Le parseur DTD ne couvre pas toutes les déclarations XML. Une déclaration qu'il omet entraîne donc une erreur de validation. Le scanner ne traite pas les SVG compressés (SVGZ) tant qu'une lecture décompressée sûre depuis le staging n'est pas définie.
