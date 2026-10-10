# Guide d'utilisation de Pigoune

*[English version](../en/README.md)*

Pigoune range tous vos éléments graphiques (icônes, logos, illustrations) dans une seule
bibliothèque bien organisée. Ce guide présente tout ce qu'il sait faire.

- [Premiers pas](#premiers-pas)
- [Organiser](#organiser)
- [Retrouver un élément](#retrouver-un-élément)
- [Regarder vos éléments](#regarder-vos-éléments)
- [Réutiliser vos éléments](#réutiliser-vos-éléments)
- [Corbeille et annulation](#corbeille-et-annulation)
- [Votre bibliothèque](#votre-bibliothèque)
- [Préférences](#préférences)
- [Raccourcis clavier](#raccourcis-clavier)
- [Questions et réponses](#questions-et-réponses)

![La fenêtre principale de Pigoune](../../data/screenshots/01.png)

## Premiers pas

### Créer une bibliothèque

Une bibliothèque est un dossier dans lequel Pigoune garde une copie de vos éléments. Sur
l'écran d'accueil, choisissez **Nouvelle bibliothèque…**, donnez-lui un nom et choisissez où
l'enregistrer. Vous pouvez en créer autant que vous voulez, par exemple une par projet ou par
client.

Pour ouvrir une bibliothèque existante, choisissez **Ouvrir une bibliothèque…**. Pigoune
rouvre la dernière bibliothèque utilisée à chaque démarrage. Quand une bibliothèque est
ouverte, le menu principal propose **Nouvelle bibliothèque…** et **Ouvrir une bibliothèque…**
pour passer à une autre, et **Bibliothèques récentes** liste les dernières bibliothèques
ouvertes, cinq par défaut. La page d'accueil les affiche aussi, pour en rouvrir une d'un
clic. Sur la page d'accueil, retirez-en une de la liste avec sa croix, ou choisissez **Effacer
la liste**, là ou dans le menu : seule la liste change, les bibliothèques elles-mêmes sont
conservées.

**Fermer la bibliothèque**, dans le menu principal, ramène à la page d'accueil. Au prochain
lancement, Pigoune s'ouvre aussi sur la page d'accueil.

### Importer des éléments

Cliquez sur le bouton **+** en haut de la barre latérale, ou appuyez sur
<kbd>Ctrl</kbd>+<kbd>I</kbd>, puis choisissez **Importer des fichiers…** ou **Importer un
dossier…**. Vous pouvez aussi simplement glisser des fichiers ou des dossiers depuis Fichiers
jusque dans la fenêtre de Pigoune. Autre possibilité : copiez des fichiers dans Fichiers (ou copiez
une image, comme une capture d'écran ou une image d'une page web), puis appuyez sur
<kbd>Ctrl</kbd>+<kbd>V</kbd> dans Pigoune. Une image collée s'appelle **Image collée** suivie de la
date et de l'heure ; vous pouvez la renommer ensuite.
Vous pouvez aussi glisser une image d'une page web sur la fenêtre, ou sur une collection de la
barre latérale : Pigoune garde ce que le
navigateur lui remet (le fichier d'origine avec Chrome ou Brave, une copie PNG de l'image avec
Firefox) et la nomme d'après le nom de fichier que le navigateur propose. Si le glissement ne
contient aucune image, un message le dit.

- Les formats pris en charge sont SVG, PNG, JPEG, WebP, AVIF, HEIC, JPEG XL, GIF, TIFF,
  BMP et ICO. Les photos HEIC s’affichent quand le système sait les décoder, ce que fait le
  paquet Flatpak ; un fichier qui ne peut pas être affiché est refusé au lieu d’être importé.
- Pigoune vérifie chaque image avant de l'importer : un fichier endommagé est signalé au lieu
  d'être ajouté.
- Les doublons sont reconnus : un fichier déjà présent n'est jamais copié deux fois. S'il est à la corbeille, l'importer le sort de la corbeille ;
  si vous importez dans une collection, il n'appartient alors qu'à cette collection.
- Quand vous importez un dossier, ses sous-dossiers deviennent des collections.
- Vos fichiers d'origine ne sont jamais modifiés ni déplacés. Vous pouvez les supprimer une
  fois importés.

Les fichiers sont importés dans la vue que vous regardez : **Tout** et **Non classés** les
gardent sans collection, et une collection sélectionnée les reçoit. Les favoris, les tags, les
collections dynamiques et la corbeille ne font que montrer des éléments, l'import y est donc
désactivé. Vous pouvez aussi déposer des fichiers directement sur **Tout**, **Non classés** ou
une collection de la barre latérale.

## Organiser

### Les collections

Les collections fonctionnent comme des dossiers et peuvent contenir d'autres collections.
Créez-en une avec le bouton **+** à côté de **Collections**, dans la barre latérale. Un clic
droit sur une collection permet d'y créer une sous-collection, de la renommer
(<kbd>F2</kbd>), de la personnaliser, de la déplacer dans une autre collection ou au premier
niveau avec **Déplacer vers…**, ou de la supprimer. Le menu est présenté de la même façon pour les
tags.

- **Personnalisez** une collection pour lui donner sa propre icône et sa propre couleur dans la
  barre latérale : clic droit, **Personnaliser…**, choisissez une couleur et une icône, puis
  **Enregistrer**. **Par défaut** remet le dossier gris.
- **Déplacez** des éléments dans une collection en les glissant dessus depuis la grille, ou,
  quand vous êtes dans une collection, avec **Déplacer vers une collection…** dans le menu du
  clic droit.
- **Ajoutez-les** à une collection sans les retirer de leur place en maintenant
  <kbd>Ctrl</kbd> pendant le glisser, ou avec **Ajouter à une collection…** dans le menu du
  clic droit. Un élément peut appartenir à plusieurs collections. Le curseur indique ce que
  vous faites : une flèche simple pour un déplacement, un **+** pour un ajout. Il change dès
  que vous appuyez sur <kbd>Ctrl</kbd> ou le relâchez, et une collection ne s'illumine pas
  quand vous y glissez des éléments qui en viennent déjà.
- **Retirez-les** de la collection affichée avec **Retirer de la collection** dans le menu du
  clic droit. Ils restent dans la bibliothèque et dans leurs autres collections.
- **Réorganisez** les collections en les glissant dans la barre latérale, ou triez-les par nom
  ou par date de création avec le bouton **⋯** à côté de **Collections**.
- **Repliez** une section de la barre latérale (Collections, Collections dynamiques ou Tags)
  d'un clic sur son titre, ou avec <kbd>Entrée</kbd> quand le titre a le focus du clavier.
  Pigoune retient les sections repliées.
- **Non classés** regroupe les éléments qui n'appartiennent à aucune collection.

Quand vous supprimez une collection, ses sous-collections le sont aussi, et les éléments qui
n'appartenaient qu'à elles partent dans la corbeille. Ceux qui appartiennent aussi à une
autre collection y restent.

### Les tags

Un tag est un seul mot de 20 caractères au plus : il décrit un élément en un mot-clé. Pour
réunir plusieurs mots, utilisez un tiret (*flat-design*). Une espace ou une virgule termine le
tag en cours de saisie, ce qui permet d'en saisir plusieurs à la fois. Ajoutez des tags depuis le
panneau de détails avec le bouton **+** à côté des tags, avec **Ajouter un tag…** dans le menu du clic
droit, ou glissez des éléments sur un tag de la barre latérale. Gardez-les une seconde au-dessus d'un
tag qui a des sous-tags : ils s'ouvrent, et vous pouvez déposer sur l'un d'eux ; la barre
latérale revient ensuite à sa place. La barre latérale montre les
tags en pastilles avec leur nombre d’éléments : cliquez sur l'une d'elles pour voir tous ses
éléments, ou faites un clic droit pour la renommer ou la supprimer. Au-delà de douze tags,
**+ N autres** affiche la suite. Renommer un tag avec le nom d'un autre les fusionne.

Les tags peuvent être imbriqués : *Sujet › Animaux › chèvre*. Tapez un chemin avec une barre
oblique, par exemple `animaux/chèvre`, pour créer ou réutiliser chaque niveau ; taper `animaux/`
propose ses sous-tags, et chaque suggestion montre ses parents. Un nom simple suivi de <kbd>Entrée</kbd> réutilise le tag de
ce nom où qu'il soit ; si plusieurs tags le portent, choisissez le bon dans la liste. Chaque niveau est un seul mot de
20 caractères au plus. Dans la barre latérale, la section Tags montre toujours ce que montre la grille : un
clic sur le nom d'une pastille affiche ce tag dans la grille, avec les éléments de ses sous-tags, et ouvre
ses sous-tags dans la barre latérale. Le chemin au-dessus des pastilles permet de remonter, et **Tags**
revient au premier niveau et affiche de nouveau toute la bibliothèque. Choisir autre chose dans la
barre latérale (Tout, une collection…) ramène la section Tags au premier niveau. La pastille de
recherche et les messages nomment un tag avec son chemin complet, par exemple *subject › animals*. Si
certains des éléments que vous aviez sélectionnés ne sont pas dans la nouvelle vue, un court message dit
combien. Le même nom peut exister dans deux branches différentes, mais
pas deux fois au même niveau. Faites un clic droit sur un tag pour **Nouveau sous-tag…**,
**Déplacer vers…**, **Fusionner dans…** et **Supprimer…**, ou glissez une pastille sur un autre tag,
sur un niveau du chemin ou sur **Tags** pour la déplacer. Supprimer un tag qui a des sous-tags
permet de les supprimer aussi ou de les remonter d'un niveau, et chaque changement s'annule. Si vous supprimez le tag affiché, Pigoune affiche
son parent, et l'annulation vous ramène sur le tag.
Chercher le nom d'un parent trouve aussi les éléments de ses sous-tags. Une bibliothèque avec des
tags imbriqués ne peut être ouverte que par Pigoune 2.5 ou plus récent ; vos tags actuels
deviennent des tags de premier niveau, sans rien perdre.

### Les favoris

Ajoutez un élément aux favoris avec l'étoile à côté de son nom, dans le panneau de
détails, appuyez sur <kbd>Ctrl</kbd>+<kbd>D</kbd>, ou glissez des éléments sur **Favoris** dans
la barre latérale. Les favoris ont leur propre entrée dans la barre latérale.

### Noms, notes et crédits

Cliquez sur le nom d'un élément dans le panneau de détails (ou appuyez sur <kbd>F2</kbd>)
pour le renommer. Le groupe **Note et crédits** permet d'écrire une note et d'indiquer la
source, la licence et l'auteur de chaque élément, ce qui est bien pratique au moment de le
réutiliser. Le groupe **Fichier**, en dessous, indique quand l’élément a été ajouté et le
nom du fichier d'origine. Chaque groupe se replie et se déplie d'un clic sur son titre, qui
affiche aussi un court résumé, et Pigoune retient ceux qui sont ouverts. Sous le nom, **Ouvrir
avec…**, **Copier** et **Exporter…** agissent directement sur l’élément.

### Plusieurs éléments à la fois

Sélectionnez plusieurs éléments avec <kbd>Ctrl</kbd>+clic, <kbd>Maj</kbd>+clic, en traçant
un rectangle depuis un espace vide de la grille, ou avec <kbd>Ctrl</kbd>+<kbd>A</kbd>. Le
panneau de détails affiche alors un résumé et permet de changer les favoris et les tags de
tous, tandis que le glisser et le menu du clic droit agissent sur toute la sélection. Un tag
porté par une partie seulement des éléments sélectionnés est entouré de pointillés, avec son
nombre (par exemple 2/5) : cliquez son nom pour l'ouvrir, ou le petit + à côté pour l'ajouter à
tous.

## Retrouver un élément

Cliquez dans le champ de recherche, appuyez sur <kbd>Ctrl</kbd>+<kbd>F</kbd>, ou commencez
simplement à taper. Pigoune cherche dans les noms, les tags, les notes, les sources, les
licences et les auteurs, sans tenir compte des majuscules, des accents ni des ligatures (*coeur* trouve
*cœur*), dans l'entrée sélectionnée de la barre latérale. Une petite pastille au début du champ de
recherche indique où elle cherche : **Partout** ou le nom de l'entrée. Cliquez dessus pour choisir
**Dans toute la bibliothèque** ou **Dans** l'entrée. Quand vous choisissez une entrée de la barre
latérale alors qu'une recherche regarde partout, la grille montre cette entrée avec votre recherche
appliquée dedans ; taper de nouveau élargit la recherche comme avant. Dans **Tout**, la corbeille et les collections
dynamiques, la pastille reste, sans menu, car la recherche y regarde toujours au même endroit. Dans une fenêtre étroite, le champ laisse la place à une loupe en haut à
droite : cliquez dessus, appuyez sur <kbd>Ctrl</kbd>+<kbd>F</kbd> ou commencez à taper, et une barre de recherche pleine largeur
s'ouvre sous la barre de titre, avec la même pastille et le bouton **Filtres**. <kbd>Échap</kbd> la ferme et efface la recherche.

- **Espace = et.** `logo chèvre` trouve les éléments qui contiennent *logo* **et** *chèvre*,
  même à des endroits différents (par exemple *logo* dans le nom et *chèvre* dans un tag).
- **Virgule = ou.** `logo, chèvre` trouve ceux qui contiennent *logo* **ou** *chèvre*.
- **Les virgules découpent la recherche en groupes** : un élément est trouvé dès qu'il
  contient tous les mots d'un groupe. `logo rouge, chèvre` trouve ce qui contient à la fois
  *logo* et *rouge*, ou bien *chèvre*.
- Un mot peut n'être qu'un morceau de mot : `chat` trouve aussi *château*.
- À partir de deux mots, ils apparaissent en pastilles sous le champ de recherche, reliées par
  **et** ou **ou**. Un clic sur un **ou** réunit les deux groupes voisins ; un clic sur un
  **et** coupe le groupe en deux à cet endroit. Dans `logo rouge, chèvre`, cliquer sur le
  **ou** donne `logo rouge chèvre` (les trois mots sont exigés) ; cliquer sur le **et** donne
  `logo, rouge, chèvre` (un seul suffit). La croix d'une pastille retire son mot. Les mêmes
  pastilles apparaissent sous **Mots à rechercher** dans la fenêtre des collections
  dynamiques.
- Une recherche utilise 8 mots au maximum : les suivants sont ignorés, et les pastilles
  l'indiquent.

La recherche ne regarde pas le format : pour ne garder que les SVG, par exemple, utilisez le
bouton **Filtres**, qui garde aussi, au choix, seulement vos favoris.

**Filtres** propose aussi treize couleurs. Pigoune note les couleurs principales de chaque
élément, jusqu'à trois qui couvrent chacune au moins 15 % de l'image visible, sans compter
les zones transparentes. Choisissez rouge et bleu pour voir les éléments surtout rouges ou
bleus. La dernière pastille, arc-en-ciel, ouvre le sélecteur de couleur de GNOME pour choisir
n'importe quelle couleur, par exemple la couleur exacte d'une charte graphique : Pigoune garde
alors les éléments dont une couleur principale en est proche. Un nouveau clic la retire. Les
couleurs s'ajoutent aux types et aux favoris : rouge et SVG montrent les SVG rouges. À la première ouverture d'une bibliothèque avec Pigoune 2.0, ses éléments sont
analysés en arrière-plan pendant que vous continuez à travailler ; un élément pas encore
analysé ne correspond à aucune couleur.

**Orientation** garde les éléments au format **Paysage**, **Portrait** ou **Carré** ; choisissez-en
deux pour voir les deux. Une image compte comme carrée quand ses côtés diffèrent de 5 % au plus.
**Adapté à mon écran** garde les images au moins aussi grandes que l'écran où se trouve
Pigoune, en vrais pixels et dans les deux sens, par exemple 1920 × 1080 ou plus : elles
remplissent l'écran sans être agrandies, donc sans flou. Les SVG sont toujours mis de côté.
Ensemble, **Paysage** et **Adapté à mon écran** trouvent de bons fonds d'écran.

Pour savoir où un élément est rangé, regardez **Collections** dans le groupe
**Organisation** du panneau de détails. Un clic sur une collection l'ouvre dans la barre
latérale et met l’élément en évidence. Un clic sur un tag fonctionne de la même façon.
**Couleurs détectées** montre les couleurs principales trouvées par Pigoune ; pour ne garder
que les éléments d'une couleur, utilisez le bouton **Filtres**.

### Les collections dynamiques

Une collection dynamique est une recherche enregistrée qui se tient à jour toute seule :
elle affiche toujours les éléments qui correspondent à ses critères, y compris ceux que vous
importez plus tard. Par exemple, *tous mes SVG favoris* ou *toutes les images en paysage
adaptées à mon écran*. Elle cherche toujours dans toute la bibliothèque, sauf la corbeille.

- **Créez-en une** avec le bouton **+** à côté de **Collections dynamiques** dans la barre
  latérale. La fenêtre est préremplie avec la recherche et les filtres en cours, et avec
  **Favoris seulement** quand **Favoris** est ouvert. Donnez-lui un nom, puis choisissez des mots à
  rechercher, des types, des orientations, des couleurs, **Adapté à mon écran** ou **Favoris
  seulement** : il faut au moins un critère. **Adapté à mon écran** suit l'écran où se trouve
  Pigoune au moment où vous ouvrez la collection dynamique.
- **Mots à rechercher** fonctionne exactement comme le champ de recherche : espaces et
  virgules, et pastilles dont les **et** et les **ou** coupent ou réunissent les groupes.
- **Ouvrez-la** depuis la barre latérale pour voir ses éléments. Vous pouvez encore chercher
  à l'intérieur.
- **Modifiez** son nom ou ses critères avec **Modifier…** dans son menu du clic droit, ou avec
  <kbd>F2</kbd>. **Supprimer…** ne supprime que la recherche enregistrée : les éléments
  restent dans la bibliothèque.
- **Réorganisez** les collections dynamiques en les faisant glisser dans la barre latérale,
  ou triez-les par nom ou par date de création avec le bouton **⋯** à côté de leur titre.

On ne peut pas déposer d’éléments sur une collection dynamique, puisque son contenu suit
ses critères.

## Regarder vos éléments

- Changez la taille des vignettes avec le curseur au-dessus de la grille, ou avec
  <kbd>Ctrl</kbd>+<kbd>+</kbd> et <kbd>Ctrl</kbd>+<kbd>-</kbd>. Le curseur fixe la taille
  minimale : les vignettes grandissent légèrement pour que chaque rangée occupe toute la
  largeur de la fenêtre.
- Triez la grille par date d'ajout, nom, type, dimensions ou poids avec le bouton de tri.
- Les GIF, PNG et WebP animés s'animent au survol, sauf si votre système est réglé pour réduire
  les animations.
- Appuyez sur <kbd>Espace</kbd> ou double-cliquez sur un élément pour ouvrir l'**aperçu
  détaillé**. Zoomez avec la molette ou avec <kbd>+</kbd> et <kbd>-</kbd>, utilisez
  <kbd>0</kbd> pour ajuster à la fenêtre et <kbd>1</kbd> pour la taille réelle, faites glisser une image zoomée ou déplacez-la avec
  <kbd>Maj</kbd> et les flèches, et choisissez
  une couleur de fond avec le bouton **Fond** de la barre du haut. Les boutons **Ouvrir
  avec…**, **Copier** et **Exporter vers…** placés à côté agissent sur l’élément affiché.
  Le bouton **Afficher les détails** ouvre le panneau de détails à côté de l’élément, pour le
  taguer, le ranger ou lui ajouter une note sans quitter l'aperçu ; Pigoune retient s'il est
  ouvert. Appuyez sur <kbd>Espace</kbd> ou <kbd>Échap</kbd> pour revenir.
- Passez à l’élément précédent ou suivant avec les flèches du clavier, avec les boutons
  fléchés qui apparaissent quand vous bougez la souris, ou par un balayage à deux doigts.
  <kbd>Début</kbd> et <kbd>Fin</kbd> mènent au premier et au dernier élément.
- Appuyez sur <kbd>F11</kbd>, ou choisissez **Plein écran** dans le menu du zoom, pour occuper
  tout l'écran ; la barre du haut revient quand vous bougez la souris ou appuyez sur <kbd>Tab</kbd>, et <kbd>Échap</kbd> quitte
  le plein écran.
- Pour un fichier ICO qui contient plusieurs tailles, des boutons sous l'image affichent chaque
  taille telle qu'elle a été dessinée (16, 32, 48, 256…).
- Pour une animation, une barre sous l'image la met en pause, la relance et la parcourt image
  par image, en indiquant l'image affichée. Au clavier, <kbd>K</kbd> met en pause ou relance,
  <kbd>,</kbd> et <kbd>.</kbd> affichent l'image précédente et suivante.
- Dans l'aperçu, faites glisser l'image pour en examiner n'importe quelle partie, même un coin
  amené au milieu de l'écran. Un contour en pointillés montre les vrais bords de l'image,
  marges transparentes comprises ; désactivez-le avec **Afficher les limites de l’image** dans
  le menu du zoom.
- À partir de 800 %, une grille légère sépare les pixels des images, pratique pour vérifier
  une icône ou du pixel art ; désactivez-la avec **Afficher la grille des pixels** dans le menu
  du zoom.
- Une bande de vignettes en bas de l'aperçu montre les éléments voisins ; cliquez sur l'une
  d'elles pour l'afficher. Elle est masquée en plein écran et dans les fenêtres étroites ;
  désactivez-la avec **Afficher la bande de vignettes** dans le menu du zoom.
- L'étoile à côté du bouton de retour ajoute l’élément affiché à vos favoris, et un clic droit
  sur l'image propose **Ouvrir avec…**, **Afficher dans Fichiers**, **Copier**, **Exporter vers…**,
  **Convertir et exporter…**, **Cadrer et définir comme fond d'écran…**, **Ajouter aux favoris** et
  **Mettre à la corbeille** pour elle. <kbd>Suppr</kbd> envoie aussi l’élément affiché à la corbeille, et l'aperçu passe à la
  suivante.

## Réutiliser vos éléments

- **Glissez** des éléments depuis la grille vers n'importe quelle application (un éditeur de
  texte, un logiciel de graphisme, une page web…). Pigoune y dépose une copie qui porte le nom
  de l’élément. Certaines applications, comme Fichiers, peuvent afficher un curseur de
  déplacement : c'est la copie remise qui se déplace, jamais l’élément de votre
  bibliothèque.
- **Copiez-les** avec <kbd>Ctrl</kbd>+<kbd>C</kbd> pour les coller ailleurs.
- **Exportez-les** dans un dossier avec **Exporter vers…** dans le menu du clic droit. Les
  fichiers existants ne sont jamais écrasés.
- **Exportez toute la bibliothèque** en dossiers ordinaires depuis l'onglet **Export** de
  **Gérer la bibliothèque** (voir plus bas) : **Exporter…** crée un dossier au nom de
  la bibliothèque, avec un sous-dossier par collection et sous-collection, exactement comme
  dans la barre latérale, et y place les fichiers d'origine.
- **Convertissez-les** avec **Convertir et exporter…** dans le menu du clic droit : choisissez PNG,
  JPEG, WebP, AVIF ou ICO, puis un dossier. JPEG et AVIF proposent un réglage de qualité ; le
  JPEG n'a pas de transparence, une couleur de fond remplit donc les zones transparentes ; les
  autres formats gardent la transparence, sauf si vous désactivez **Garder la transparence**
  pour utiliser aussi une couleur de fond. Choisissez la largeur et la hauteur, en pixels ou en
  pourcentage ; 100 % garde la taille d'origine. Cadenas fermé, les proportions sont gardées :
  changer un côté ajuste l'autre, et plusieurs images de formes différentes tiennent chacune
  dans la largeur et la hauteur indiquées. Ouvrez-le pour étirer librement une image. Pour
  adapter plusieurs images à votre écran d'un coup, activez **Taille de mon écran** : chaque
  image prend exactement la taille de l'écran où se trouve Pigoune, par exemple 1920 × 1080.
  **Remplir l'écran** couvre tout l'écran et coupe les bords qui dépassent, autour du centre ;
  **Image entière** garde toute l'image et remplit les bandes avec la couleur de fond, ou les
  laisse transparentes quand la transparence est gardée. Un SVG
  reste net à toutes les tailles ; une image en pixels agrandie devient floue. Un fichier
  redimensionné porte sa taille dans son nom, par exemple `logo-512x384.png`. Depuis l'aperçu,
  une animation en pause sur une image exporte cette image, nommée par exemple
  `spinner-image-3.png`. Pour l'ICO, cochez les tailles à inclure (16, 32, 48 et 256 par défaut)
  : elles vont toutes dans un seul fichier d'icône. Pigoune retient votre format, votre qualité,
  votre fond et votre unité pour le prochain export, et la taille repart toujours de l'original.
  L'original reste intact dans la bibliothèque, et les fichiers existants ne sont jamais
  écrasés.
- **Ouvrez** un élément dans une autre application, un logiciel de retouche par exemple,
  avec **Ouvrir avec…** dans le menu du clic droit. L'application reçoit une copie : votre
  bibliothèque reste intacte, enregistrez donc vos modifications sous un nouveau nom et
  importez-les si vous voulez les garder.
- **Retrouvez le fichier sur le disque** avec **Afficher dans Fichiers** dans le menu du clic
  droit : votre gestionnaire de fichiers s'ouvre sur le fichier stocké dans la bibliothèque.
  C'est le vrai fichier : ne le déplacez, ne le renommez et ne le modifiez pas depuis là ; sinon,
  le bilan de santé de la bibliothèque le signalera.
- **Utilisez une image comme fond d'écran** avec **Cadrer et définir comme fond d'écran…** dans
  le menu du clic droit de la grille ou de l'aperçu. Une grande fenêtre montre un écran virtuel
  à la résolution réelle de l'écran où se trouve Pigoune, par exemple 1920 × 1080, avec l'image
  cadrée comme GNOME le ferait. Faites glisser l'image pour la déplacer, zoomez avec la molette,
  le curseur ou <kbd>+</kbd> et <kbd>-</kbd>, et ajustez-la avec les flèches (<kbd>Maj</kbd> pour
  aller plus loin). **Remplir** revient au cadrage de GNOME, **Entière** montre toute l'image et
  **100 %** montre un pixel de l'image par pixel de l'écran. **Miroir** retourne l'image de gauche
  à droite, dans le fond d'écran seulement, **Règle des tiers** affiche la grille des tiers pour placer le
  sujet, et **Magnétisme**, actif au départ, accroche l'image glissée au centre et aux bords de
  l'écran, une ligne bleue montrant où. Ce qui dépasse de l'écran reste visible, en pâle. Quand l'image est agrandie au-delà de 100 %, une pastille sur l'écran prévient
  qu'elle sera floue. Le bouton plein écran, ou <kbd>F11</kbd>, montre l'écran virtuel en vraie
  grandeur, comme une simulation que rappelle une courte pastille ; <kbd>Échap</kbd> pour revenir.
- **Assombrir**, sous **Image**, assombrit tout le fond d'écran, jusqu'à 60 %, pour que les
  icônes et les barres restent lisibles ; comme **Miroir**, il ne change que le fond d'écran,
  jamais l’élément. Avec plusieurs écrans, **Préparer pour** choisit celui pour lequel le fond
  d'écran est fabriqué : GNOME affiche le même fond d'écran sur tous les écrans et l'adapte aux
  autres.
- **Simulation du bureau** dessine une imitation des barres de votre bureau sur l'écran
  virtuel : GNOME, KDE Plasma, Cinnamon, Xfce, MATE, COSMIC ou Budgie, celui que vous utilisez
  étant choisi d'office. Désactivez-la pour voir l'image seule.
- Sous **Espace vide**, choisissez ce qui remplit l'écran autour d'une image plus petite : une
  **Couleur**, noire au départ, un **Dégradé**, le **Flou**, la même image
  agrandie et floutée derrière elle, ou la **Mosaïque**, l'image répétée sur tout l'écran,
  pratique pour les motifs. Les couleurs se choisissent parmi des pastilles qui reprennent les
  couleurs principales de l'image, le noir et le blanc, et la pastille arc-en-ciel ouvre le
  sélecteur de couleur de GNOME. Un dégradé part des deux couleurs principales de
  l'image, du haut vers le bas ; **Angle** le fait tourner (0° vers le haut, 90° vers la droite,
  180° vers le bas), les boutons **Couleur de début** et **Couleur de fin** ouvrent le sélecteur
  de couleur de GNOME, et **Inverser les couleurs** les échange.
- **Définir comme fond d'écran** fabrique une image exactement à la taille de l'écran : le
  bureau montre exactement ce que vous avez cadré. Une petite fenêtre montre la préparation,
  puis GNOME vous demande de confirmer. Cochez **Ajouter aussi à la bibliothèque la version
  modifiée** pour garder cette image comme nouvel élément.

## Corbeille et annulation

Appuyez sur <kbd>Suppr</kbd>, ou choisissez **Mettre à la corbeille** dans le menu du clic
droit, pour envoyer des éléments à la corbeille. Rien n'est perdu tant que vous ne la videz
pas : ouvrez **Corbeille**, en bas de la barre latérale, pour restaurer des éléments, avec
**Restaurer** dans le menu du clic droit ou dans le panneau de détails, ou pour la vider
définitivement. Les éléments de la corbeille peuvent encore être copiés, exportés ou ouverts
avec une autre application, mais pas modifiés.

La plupart des modifications s'annulent avec <kbd>Ctrl</kbd>+<kbd>Z</kbd> ou avec le bouton
**Annuler** du message qui apparaît après une action : mise à la corbeille, déplacement, tags,
favoris, renommage, notes et crédits, mais aussi création, renommage, personnalisation,
déplacement ou suppression d'une collection, et création, modification, réorganisation ou
suppression d'une collection dynamique. Les imports ne s'annulent pas, et vider la corbeille
efface l'historique des annulations. Si un import sort un élément de la corbeille, sa mise à la
corbeille précédente ne figure plus dans l'historique des annulations. La création d'une collection ne s'annule que tant qu'elle
est vide : une fois des éléments importés dedans, utilisez plutôt **Supprimer…**. Quand une
annulation n'est plus possible, Pigoune le dit, laisse ce changement tel quel, et le
<kbd>Ctrl</kbd>+<kbd>Z</kbd> suivant annule le changement d'avant. Un message qui suit une action ne
compte que ce qui a vraiment changé : les éléments déjà dans la collection ou déjà étiquetés
ne sont pas comptés.

## Votre bibliothèque

Une bibliothèque est un simple dossier dont le nom se termine par `.pigoune`. Il contient :

- `files/`, une copie de chaque élément, avec son nom de fichier d'origine, rangée dans de
  petits sous-dossiers pour que même une très grande bibliothèque reste facile à manipuler ;
- `library.db`, la base de données qui contient les collections, les tags et toutes les
  autres informations ;
- `cache/`, les vignettes, que Pigoune peut refaire à tout moment.

Comme tout se trouve dans ce dossier, une bibliothèque est **portable** : copiez-la sur un
disque externe ou sur un autre ordinateur et ouvrez-la là-bas avec Pigoune. Pour la
sauvegarder, copiez le dossier entier pendant que Pigoune est fermé.

Une bibliothèque ne peut être ouverte que dans une seule fenêtre de Pigoune à la fois. Si vous
la gardez dans un dossier synchronisé (Nextcloud, Syncthing…), fermez Pigoune sur un
ordinateur avant de l'ouvrir sur un autre.

Choisissez **Gérer la bibliothèque** dans le menu principal pour voir ce que contient la
bibliothèque ouverte et vous en occuper, en cinq onglets :

- **Vue d'ensemble** montre le nom et l'emplacement de la bibliothèque, le nombre d’éléments, de
  collections, de tags et de favoris, et le **Palmarès** : l’élément le plus lourd, le plus
  grand, le plus récent et le plus ancien ; cliquez sur l'un d'eux pour le voir dans la
  grille.
- **Contenu** montre dans un graphique en anneau comment la place se répartit entre les formats,
  en poids ou en nombre d’éléments, le nombre d'images animées, de SVG et d’éléments dans la
  corbeille, et les couleurs détectées avec le nombre d’éléments dont chacune est une couleur
  principale.
- **Stockage** montre dans une seule barre la place de la bibliothèque sur son disque, répartie
  entre les éléments, les vignettes, la base de données et la corbeille, à côté des autres
  fichiers et de l'espace libre. En dessous : son emplacement, avec des boutons pour le copier ou
  ouvrir son dossier, si ce disque est amovible, la place prise par les vignettes, sa date de
  création et les versions de Pigoune capables de l'ouvrir. **Vider**, à côté des vignettes,
  libère leur place ; elles sont recréées quand on en a besoin.
- **Santé** compare les fichiers du disque avec la base de données de la bibliothèque.
  **Vérifier** lit chaque fichier, ceux de la corbeille compris, et liste les fichiers manquants,
  les fichiers endommagés (leur contenu ne correspond plus à ce qui a été importé) et les fichiers non référencés (ceux du
  dossier de la bibliothèque que la base de données ne connaît pas). La vérification se fait en arrière-plan,
  peut être annulée, et ne modifie ni ne supprime jamais rien d'elle-même. Pour un fichier non
  référencé, **Ajouter** (ou **Tout ajouter**) le remet dans la bibliothèque, dans Non classés, sans déplacer
  le fichier ; Pigoune explique pourquoi quand un fichier ne peut pas être ajouté, par exemple
  s'il n'est pas une image. Pour un fichier manquant ou endommagé, **Remplacer…** permet de choisir
  une copie du fichier d'origine ; Pigoune ne l'accepte que si son contenu est identique à ce qui
  a été importé, puis la remet en place en conservant le nom, les tags et les collections de
  l’élément. Pour un fichier manquant dont il ne reste aucune copie, **Retirer…** retire
  l’élément de la bibliothèque, après une confirmation qui précise que c'est
  définitif ; un fichier endommagé, qui existe encore, ne le propose jamais. Un fichier endommagé
  propose à la place **Mettre à la corbeille**, qu'on peut annuler depuis le message qui
  apparaît. Les éléments qui sont dans la corbeille ne sont pas signalés : leurs fichiers
  partent avec la corbeille, et si vous en restaurez un, la vérification suivante le regarde
  de nouveau.
- **Export** copie tous les éléments dans un dossier de votre choix. Pigoune crée un dossier
  au nom de la bibliothèque, avec un sous-dossier par collection et sous-collection, et y place
  les fichiers d'origine ; la page montre un exemple tiré de votre propre bibliothèque. Les
  éléments qui n'appartiennent à aucune collection vont dans un dossier **Non classés**, et
  un élément qui appartient à plusieurs collections est copié dans chacune. Les éléments à
  la corbeille sont laissés de côté. Les tags, notes et favoris restent dans Pigoune : ce n'est
  donc pas une sauvegarde complète, pour cela copiez le dossier de la bibliothèque. Rien n'est
  jamais écrasé : si un dossier du même nom existe déjà, l'export est créé sous le nom
  `Nom (2)`. Le bouton est grisé quand la bibliothèque est vide.

## Préférences

Ouvrez les **Préférences** depuis le menu principal, ou appuyez sur
<kbd>Ctrl</kbd>+<kbd>,</kbd>. Les réglages sont répartis sur trois pages ; la loupe en haut
retrouve un réglage par son nom.

**Général**

- **Langue de l'interface** : Pigoune suit la langue de votre système ; choisissez une autre des
  langues dans lesquelles Pigoune est traduit pour l'utiliser à la place. Le changement
  s'applique au prochain démarrage de Pigoune, et **Redémarrer** dans le message qui apparaît le
  fait tout de suite. Quelques fenêtres fournies par GNOME, comme celle qui permet de choisir un
  dossier, peuvent rester dans la langue du système.
- **Rouvrir la dernière bibliothèque** : au lancement, Pigoune ouvre la bibliothèque laissée
  ouverte. Désactivez-le pour partir de la page d'accueil et choisir une bibliothèque à chaque
  fois.
- **Bibliothèques récentes** : combien de bibliothèques récentes le menu principal et la page
  d'accueil proposent, de 0 à 8. Choisissez 0 pour désactiver la liste.
- **Rouvrir la dernière entrée** : une bibliothèque s'ouvre sur l'entrée utilisée la dernière
  fois plutôt que sur **Tout**.
- **Afficher les nouveautés après une mise à jour** : au premier démarrage d'une nouvelle
  version, une courte fenêtre présente ses principales nouveautés. Désactivez-la si vous
  préférez ne pas la voir ; la fenêtre À propos garde les nouveautés de la version en cours.
- **Confirmer avant de vider la corbeille**.
- **Vider automatiquement la corbeille** : les éléments sont supprimés définitivement après
  30 jours dans la corbeille.

**Affichage**

- **Afficher le nom des éléments** sous chaque vignette de la grille. Quand les noms sont
  masqués, survolez une vignette pour voir son nom.
- **Afficher les formats** : une pastille indique le format de chaque élément (SVG, PNG…)
  sur sa vignette.
- **Fond des vignettes** : blanc, gris, noir ou damier derrière les vignettes, pour voir les
  images blanches ou noires et les zones transparentes.
- **Animer les vignettes au survol** : désactivez-le si les vignettes qui bougent vous
  distraient ; les animations se jouent toujours dans le panneau de détails et l'aperçu.
- **Afficher le nombre d’éléments** à côté de chaque entrée de la barre latérale. Pendant une
  recherche ou un filtre, les nombres ne comptent que les éléments qui correspondent, et les tags
  qui n’en ont aucun sont estompés.
- **Afficher les tags** : désactivez-le pour masquer la section des tags de la barre latérale.
  Les tags restent visibles dans le panneau de détails, et la recherche les trouve toujours.
- **Afficher les collections dynamiques** : désactivez-le pour masquer la section des
  collections dynamiques de la barre latérale. Vos collections dynamiques sont conservées
  et reviennent quand vous le réactivez.

**Comportement**

- **Ouvrir les éléments au double-clic** : un double-clic ouvre l’élément dans son
  application par défaut au lieu de l'aperçu. La touche <kbd>Espace</kbd> ouvre toujours
  l'aperçu.
- **Rechercher dans toute la bibliothèque** : la recherche et les filtres regardent partout,
  et pas seulement dans l'entrée sélectionnée de la barre latérale, sauf dans la corbeille et
  dans les collections dynamiques. Il est activé au
  premier démarrage, et la pastille du champ de recherche le change à tout moment.

Pigoune suit le style et la couleur d'accentuation choisis dans les **Paramètres** de GNOME,
rubrique **Apparence** : style clair ou sombre, et couleur des sélections, des interrupteurs et
des surlignages.

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
| Coller des fichiers ou une image pour les importer | <kbd>Ctrl</kbd>+<kbd>V</kbd> |
| Renommer | <kbd>F2</kbd> |
| Ajouter aux favoris ou en retirer | <kbd>Ctrl</kbd>+<kbd>D</kbd> |
| Mettre à la corbeille | <kbd>Suppr</kbd> |
| Renommer ou modifier l'entrée sélectionnée dans la barre latérale | <kbd>F2</kbd> |
| Supprimer la collection sélectionnée dans la barre latérale | <kbd>Suppr</kbd> |
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
Non. Pigoune fonctionne entièrement hors ligne, sans compte et sans télémétrie. La recherche de
mises à jour est faite par Flatpak, qui demande seulement au dépôt de Pigoune si une nouvelle
version existe.

**Comment mettre Pigoune à jour ?**
Logiciels s'en charge : il propose chaque nouvelle version, et l'installe tout seul si les mises
à jour automatiques sont activées. `flatpak update` l'installe aussi. Si Pigoune reste ouvert,
une bannière en haut de la fenêtre l'annonce aussi dans la demi-heure : cliquez sur **Mettre à
jour**, attendez la fin de l'installation, puis cliquez sur **Redémarrer**.

**Pourquoi Pigoune dit-il qu'un fichier est illisible ?**
Le fichier est endommagé, ou n'est pas vraiment dans le format qu'indique son nom. Pigoune vérifie
le contenu de chaque image, pas seulement son extension.

**J'ai importé deux fois le même fichier. Où est la deuxième copie ?**
Il n'y en a pas : Pigoune reconnaît les doublons et ajoute plutôt l’élément existant à la
collection visée.

**Comment signaler un bug ou proposer une idée ?**
Ouvrez une issue sur [GitHub](https://github.com/Gor3pig/Pigoune/issues).
