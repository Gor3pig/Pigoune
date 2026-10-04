# Guide d'utilisation de Pigoune

*[English version](../en/README.md)*

Pigoune range toutes vos ressources graphiques (icônes, logos, illustrations) dans une seule
bibliothèque bien organisée. Ce guide présente tout ce qu'il sait faire.

- [Premiers pas](#premiers-pas)
- [Organiser](#organiser)
- [Retrouver une ressource](#retrouver-une-ressource)
- [Regarder vos ressources](#regarder-vos-ressources)
- [Réutiliser vos ressources](#réutiliser-vos-ressources)
- [Corbeille et annulation](#corbeille-et-annulation)
- [Votre bibliothèque](#votre-bibliothèque)
- [Préférences](#préférences)
- [Raccourcis clavier](#raccourcis-clavier)
- [Questions et réponses](#questions-et-réponses)

![La fenêtre principale de Pigoune](../../data/screenshots/01.png)

## Premiers pas

### Créer une bibliothèque

Une bibliothèque est un dossier dans lequel Pigoune garde une copie de vos ressources. Sur
l'écran d'accueil, choisissez **Créer une bibliothèque**, donnez-lui un nom et choisissez où
l'enregistrer. Vous pouvez en créer autant que vous voulez, par exemple une par projet ou par
client.

Pour ouvrir une bibliothèque existante, choisissez **Ouvrir une bibliothèque**. Pigoune
rouvre la dernière bibliothèque utilisée à chaque démarrage.

### Importer des ressources

Cliquez sur le bouton **+** en haut de la barre latérale, ou appuyez sur
<kbd>Ctrl</kbd>+<kbd>I</kbd>, puis choisissez **Importer des fichiers…** ou **Importer un
dossier…**. Vous pouvez aussi simplement glisser des fichiers ou des dossiers depuis Fichiers
jusque dans la fenêtre de Pigoune.

- Les formats pris en charge sont SVG, PNG, JPEG, WebP, GIF et ICO.
- Pigoune vérifie chaque image avant de l'importer : un fichier abîmé est signalé au lieu
  d'être ajouté.
- Les doublons sont reconnus : un fichier déjà présent n'est jamais copié deux fois.
- Quand vous importez un dossier, ses sous-dossiers deviennent des collections.
- Vos fichiers d'origine ne sont jamais modifiés ni déplacés. Vous pouvez les supprimer une
  fois importés.

Si une collection ou un tag est sélectionné dans la barre latérale, les fichiers importés y
vont directement. Vous pouvez aussi déposer des fichiers directement sur une collection ou un
tag de la barre latérale.

## Organiser

### Les collections

Les collections fonctionnent comme des dossiers et peuvent contenir d'autres collections.
Créez-en une avec le bouton **+** à côté de **Collections**, dans la barre latérale. Un clic
droit sur une collection permet d'y créer une sous-collection, de la renommer
(<kbd>F2</kbd>) ou de la supprimer.

- **Déplacez** des ressources dans une collection en les glissant dessus depuis la grille.
- **Ajoutez-les** à une collection sans les retirer de leur place en maintenant
  <kbd>Ctrl</kbd> pendant le glisser, ou avec **Ajouter à une collection…** dans le menu du
  clic droit. Une ressource peut appartenir à plusieurs collections.
- **Réorganisez** les collections en les glissant dans la barre latérale, ou triez-les par nom
  ou par date de création avec le bouton de tri à côté de **Collections**.
- **Non classés** regroupe les ressources qui n'appartiennent à aucune collection.

Quand vous supprimez une collection, ses sous-collections le sont aussi, et les ressources qui
n'appartenaient qu'à elles partent dans la corbeille. Celles qui appartiennent aussi à une
autre collection y restent.

### Les tags

Les tags décrivent vos ressources avec les mots de votre choix. Ajoutez-en depuis le panneau
de détails avec le bouton **+** à côté des tags, ou glissez des ressources sur un tag de la
barre latérale. Cliquez sur un tag de la barre latérale pour voir toutes ses ressources, ou
faites un clic droit pour le renommer ou le supprimer. Renommer un tag avec le nom d'un autre
les fusionne.

### Les favoris

Marquez une ressource comme favorite avec l'étoile à côté de son nom, dans le panneau de
détails, ou appuyez sur <kbd>Ctrl</kbd>+<kbd>D</kbd>. Les favoris ont leur propre entrée dans
la barre latérale.

### Noms, notes et crédits

Cliquez sur le nom d'une ressource dans le panneau de détails (ou appuyez sur <kbd>F2</kbd>)
pour la renommer. La liste **Informations** permet aussi d'écrire une note et d'indiquer la
source, la licence et l'auteur de chaque ressource, ce qui est bien pratique au moment de la
réutiliser.

### Plusieurs ressources à la fois

Sélectionnez plusieurs ressources avec <kbd>Ctrl</kbd>+clic, <kbd>Maj</kbd>+clic, en traçant
un rectangle depuis un espace vide de la grille, ou avec <kbd>Ctrl</kbd>+<kbd>A</kbd>. Le
panneau de détails affiche alors un résumé et permet de changer les favoris et les tags de
toutes, tandis que le glisser et le menu du clic droit agissent sur toute la sélection. Un tag
porté par une partie seulement des ressources sélectionnées est entouré de pointillés : un
clic dessus l'ajoute à toutes.

## Retrouver une ressource

Cliquez dans le champ de recherche, appuyez sur <kbd>Ctrl</kbd>+<kbd>F</kbd>, ou commencez
simplement à taper. Pigoune cherche dans les noms, les tags, les notes, les sources, les
licences et les auteurs, sans tenir compte des majuscules ni des accents, dans l'entrée
sélectionnée de la barre latérale. Sélectionnez **Tout** pour chercher dans toute la
bibliothèque.

Le bouton **Filtres** ne garde que certains formats, ou seulement vos favoris.

Pour savoir où une ressource est rangée, regardez **Rangée dans**, en bas du panneau de
détails. Un clic sur une collection l'ouvre dans la barre latérale et met la ressource en
évidence. Un clic sur un tag du panneau de détails fonctionne de la même façon.

## Regarder vos ressources

- Changez la taille des vignettes avec le curseur au-dessus de la grille, ou avec
  <kbd>Ctrl</kbd>+<kbd>+</kbd> et <kbd>Ctrl</kbd>+<kbd>-</kbd>. Le curseur fixe la taille
  minimale : les vignettes grandissent légèrement pour que chaque rangée occupe toute la
  largeur de la fenêtre.
- Triez la grille par date d'ajout, nom, type, dimensions ou poids avec le bouton de tri.
- Les GIF animés s'animent au survol.
- Appuyez sur <kbd>Espace</kbd> ou double-cliquez sur une ressource pour ouvrir l'**aperçu
  détaillé**. Zoomez avec la molette ou avec <kbd>+</kbd> et <kbd>-</kbd>, utilisez
  <kbd>0</kbd> pour ajuster à la fenêtre et <kbd>1</kbd> pour la taille réelle, passez à la
  ressource précédente ou suivante avec les flèches du clavier ou avec les boutons fléchés qui
  apparaissent quand vous bougez la souris, et choisissez une couleur de fond dans la
  barre du haut. Appuyez sur <kbd>Espace</kbd> ou <kbd>Échap</kbd> pour revenir.

## Réutiliser vos ressources

- **Glissez** des ressources depuis la grille vers n'importe quelle application (un éditeur de
  texte, un logiciel de graphisme, une page web…). Pigoune y dépose une copie qui porte le nom
  de la ressource.
- **Copiez-les** avec <kbd>Ctrl</kbd>+<kbd>C</kbd> pour les coller ailleurs.
- **Exportez-les** dans un dossier avec **Exporter vers…** dans le menu du clic droit. Les
  fichiers existants ne sont jamais écrasés.
- **Ouvrez** une ressource dans une autre application, un logiciel de retouche par exemple,
  avec **Ouvrir avec…** dans le menu du clic droit. L'application reçoit une copie : votre
  bibliothèque reste intacte, enregistrez donc vos modifications sous un nouveau nom et
  importez-les si vous voulez les garder.

## Corbeille et annulation

Appuyez sur <kbd>Suppr</kbd>, ou choisissez **Mettre à la corbeille** dans le menu du clic
droit, pour envoyer des ressources à la corbeille. Rien n'est perdu tant que vous ne la videz
pas : ouvrez **Corbeille** dans la barre latérale pour restaurer des ressources ou la vider
définitivement.

La plupart des modifications s'annulent avec <kbd>Ctrl</kbd>+<kbd>Z</kbd> ou avec le bouton
**Annuler** du message qui apparaît après une action : mise à la corbeille, déplacement, tags,
favoris, renommage, notes et crédits. Les imports ne s'annulent pas, et vider la corbeille
efface l'historique des annulations.

## Votre bibliothèque

Une bibliothèque est un simple dossier dont le nom se termine par `.pigoune`. Il contient :

- `files/`, une copie de chaque ressource, avec son nom de fichier d'origine ;
- `library.db`, la base de données qui contient les collections, les tags et toutes les
  autres informations ;
- `cache/`, les vignettes, que Pigoune peut refaire à tout moment.

Comme tout se trouve dans ce dossier, une bibliothèque est **portable** : copiez-la sur un
disque externe ou sur un autre ordinateur et ouvrez-la là-bas avec Pigoune. Pour la
sauvegarder, copiez le dossier entier pendant que Pigoune est fermé.

Une bibliothèque ne peut être ouverte que dans une seule fenêtre de Pigoune à la fois. Si vous
la gardez dans un dossier synchronisé (Nextcloud, Syncthing…), fermez Pigoune sur un
ordinateur avant de l'ouvrir sur un autre.

## Préférences

Ouvrez les **Préférences** depuis le menu principal, ou appuyez sur
<kbd>Ctrl</kbd>+<kbd>,</kbd> :

- **Afficher le nom des ressources** sous chaque vignette de la grille. Quand les noms sont
  masqués, survolez une vignette pour voir son nom.
- **Afficher le nombre de ressources** à côté de chaque entrée de la barre latérale.
- **Rouvrir la dernière entrée** : une bibliothèque s'ouvre sur l'entrée utilisée la dernière
  fois plutôt que sur **Tout**.
- **Confirmer avant de vider la corbeille**.
- **Vider automatiquement la corbeille** : les ressources sont supprimées définitivement après
  30 jours dans la corbeille.

## Raccourcis clavier

Appuyez sur <kbd>Ctrl</kbd>+<kbd>?</kbd> pour voir tous les raccourcis de Pigoune.

| Action | Raccourci |
|---|---|
| Nouvelle bibliothèque | <kbd>Ctrl</kbd>+<kbd>N</kbd> |
| Ouvrir une bibliothèque | <kbd>Ctrl</kbd>+<kbd>O</kbd> |
| Importer des fichiers | <kbd>Ctrl</kbd>+<kbd>I</kbd> |
| Rechercher | <kbd>Ctrl</kbd>+<kbd>F</kbd> |
| Aperçu détaillé | <kbd>Espace</kbd> |
| Tout sélectionner / tout désélectionner | <kbd>Ctrl</kbd>+<kbd>A</kbd> / <kbd>Échap</kbd> |
| Copier | <kbd>Ctrl</kbd>+<kbd>C</kbd> |
| Renommer | <kbd>F2</kbd> |
| Ajouter aux favoris ou en retirer | <kbd>Ctrl</kbd>+<kbd>D</kbd> |
| Mettre à la corbeille | <kbd>Suppr</kbd> |
| Annuler | <kbd>Ctrl</kbd>+<kbd>Z</kbd> |
| Vignettes plus grandes / plus petites | <kbd>Ctrl</kbd>+<kbd>+</kbd> / <kbd>Ctrl</kbd>+<kbd>-</kbd> |
| Menu contextuel | <kbd>Menu</kbd> ou <kbd>Maj</kbd>+<kbd>F10</kbd> |
| Aide | <kbd>F1</kbd> |
| Préférences | <kbd>Ctrl</kbd>+<kbd>,</kbd> |
| Quitter | <kbd>Ctrl</kbd>+<kbd>Q</kbd> |

## Questions et réponses

**Puis-je supprimer mes fichiers d'origine après les avoir importés ?**
Oui. Pigoune travaille sur ses propres copies, rangées dans la bibliothèque.

**Pigoune envoie-t-il quelque chose sur internet ?**
Non. Pigoune fonctionne entièrement hors ligne, sans compte et sans télémétrie.

**Pourquoi Pigoune dit-il qu'un fichier est illisible ?**
Le fichier est abîmé, ou n'est pas vraiment dans le format qu'indique son nom. Pigoune vérifie
le contenu de chaque image, pas seulement son extension.

**J'ai importé deux fois le même fichier. Où est la deuxième copie ?**
Il n'y en a pas : Pigoune reconnaît les doublons et ajoute plutôt la ressource existante à la
collection visée.

**Comment signaler un bug ou proposer une idée ?**
Ouvrez une issue sur [GitHub](https://github.com/Gor3pig/Pigoune/issues).
