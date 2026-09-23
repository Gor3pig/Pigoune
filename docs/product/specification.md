# Pigoune — Spécification produit

> Français — version de référence · [English](specification.en.md)

**P**lateforme d’**I**cônes et **G**raphismes — **O**rganisée, **U**nifiée, **N**ative et **É**légante.

**Version :** 0.1  
**Statut :** Base produit initiale

## 1. Vision

Pigoune est une bibliothèque personnelle d’assets graphiques conçue spécifiquement pour Linux et GNOME.

Elle permet de collecter, conserver, organiser, rechercher, inspecter et réutiliser des ressources graphiques sans considérer l’arborescence du système de fichiers comme principal modèle d’organisation.

Pigoune n’est pas :

- un gestionnaire de fichiers généraliste ;
- une visionneuse d’images généraliste ;
- un éditeur d’images ;
- un DAM collaboratif ;
- un service cloud ;
- un service de synchronisation.

Un asset importé devient un objet autonome de la bibliothèque.

## 2. Plateforme

Cible principale :

- Linux
- GNOME
- GTK 4
- Libadwaita
- Flatpak

La génération de bureau de référence est GNOME 50.

Pigoune vise volontairement Linux/GNOME plutôt qu’une interface multiplateforme.

## 3. Confidentialité et connectivité

Pigoune est conçu pour fonctionner entièrement hors ligne.

Il ne nécessite aucun :

- compte utilisateur ;
- service cloud ;
- système de télémétrie ;
- analytics ;
- service d’IA distant ;
- upload automatique ;
- système de synchronisation automatique.

Une URL enregistrée dans les métadonnées d’un asset reste une simple donnée. Pigoune ne la contacte jamais automatiquement.

## 4. Propriété des assets

Importer un fichier copie son contenu dans la bibliothèque.

Après un import réussi, le fichier source externe et l’asset de la bibliothèque n’entretiennent plus aucune relation.

Déplacer, renommer, modifier ou supprimer le fichier externe ne doit pas affecter l’asset stocké par Pigoune.

Le chemin source ne fait pas partie de l’identité de l’asset.

## 5. Préservation binaire

Le contenu binaire importé est conservé bit pour bit.

Pigoune peut générer des données dérivées telles que :

- miniatures ;
- aperçus ;
- index de recherche ;
- couleurs dominantes ;
- métadonnées extraites.

Les données dérivées ne remplacent ni ne modifient jamais l’objet binaire conservé.

Pigoune n’est pas un éditeur d’images.

## 6. Portabilité de la bibliothèque

Une bibliothèque Pigoune est un répertoire autonome.

L’expérience de la version 1.0 repose sur une seule bibliothèque principale active.

Dans l’usage normal, une bibliothèque déjà configurée est ouverte directement.

Au premier lancement ou lorsqu’aucune bibliothèque n’est disponible, une vue d’accueil permet de :

- créer une bibliothèque ;
- ouvrir une bibliothèque existante ;
- restaurer une sauvegarde.

L’application gère automatiquement un emplacement par défaut.

Une bibliothèque peut néanmoins être créée, déplacée ou ouverte depuis un emplacement choisi par l’utilisateur. Son dossier reste totalement autonome.

L’architecture ne doit pas empêcher la gestion de plusieurs bibliothèques à l’avenir, mais leur gestion effective reste postérieure à la version 1.0.

Les données essentielles de la bibliothèque y sont conservées, notamment :

- base de métadonnées ;
- objets graphiques stockés ;
- collections ;
- tags ;
- familles ;
- notes ;
- données de récupération ;
- configuration propre à la bibliothèque.

Les données jetables comme les miniatures et aperçus restent en dehors de la bibliothèque et peuvent être reconstruites.

Copier le répertoire d’une bibliothèque dans un état cohérent doit suffire pour la déplacer ou la sauvegarder.

## 7. Fiabilité

L’ordre de priorité est :

1. intégrité ;
2. récupération ;
3. prévisibilité ;
4. performance.

Les optimisations de performances ne doivent jamais affaiblir la sécurité de la bibliothèque.

## 8. Identité et stockage des assets

Un asset logique possède son propre UUID.

Le contenu binaire physique est identifié séparément par son SHA-256.

Plusieurs assets logiques peuvent référencer le même objet physique immuable lorsque leurs contenus sont identiques.

Cela permet une déduplication physique transparente tout en conservant des métadonnées indépendantes.

La déduplication physique est limitée à une bibliothèque. Il n’existe aucune référence physique partagée entre deux bibliothèques.

## 9. Organisation

Un asset appartient à zéro ou une collection.

Les assets sans collection apparaissent dans `Sans collection`.

Les collections sont hiérarchiques.

Les tags sont plats et peuvent être combinés pour le filtrage.

Les tags peuvent être renommés, fusionnés et supprimés globalement.

## 10. Familles et variantes

Plusieurs fichiers représentant des variantes d’une même ressource graphique logique peuvent être regroupés dans une famille.

Une famille possède des métadonnées communes telles que :

- nom d’affichage ;
- collection ;
- tags ;
- statut favori ;
- notes ;
- informations de source et licence.

Chaque variante conserve ses propres propriétés physiques.

Une variante est désignée comme représentation principale.

## 11. Interface principale

La disposition principale comporte :

- une barre latérale de navigation ;
- une vue centrale des assets ;
- un inspecteur rétractable.

La vue des assets prend en charge :

- le mode grille ;
- le mode liste.

La barre latérale et l’inspecteur peuvent être repliés indépendamment.

L’interface s’adapte aux fenêtres plus étroites.

## 12. Interaction avec les assets

Les interactions principales comprennent :

- sélection par clic simple ;
- vue détail par double-clic ;
- aperçu rapide avec la barre Espace ;
- navigation clavier ;
- sélection multiple ;
- glisser-déposer ;
- import et export via le presse-papiers.

Les vignettes utilisent des cases uniformes et ne recadrent jamais l’aperçu de l’asset.

## 13. Recherche

La recherche est locale.

Elle prend en charge :

- correspondances partielles ;
- recherche insensible à la casse ;
- recherche insensible aux accents ;
- plusieurs termes ;
- légère tolérance aux fautes ;
- classement par pertinence.

La recherche peut porter sur la collection courante ou sur toute la bibliothèque.

Les filtres exploitent des métadonnées pertinentes telles que :

- tags ;
- format ;
- favoris ;
- couleur dominante ;
- orientation ;
- dimensions.

Les recherches enregistrées sont des vues dynamiques distinctes des collections.

## 14. Import

Un fichier entrant est validé avant de devenir un asset.

Le pipeline d’import de base est :

1. ingestion temporaire ;
2. calcul du SHA-256 ;
3. validation du format ;
4. décodage sécurisé ;
5. extraction des métadonnées essentielles ;
6. création de l’asset.

Les fichiers invalides ne deviennent pas des assets de la bibliothèque.

Les imports importants s’exécutent en arrière-plan et restent annulables.

## 15. Formats pris en charge

Les formats pleinement pris en charge sont :

- PNG
- JPEG
- SVG
- ICO
- ICNS
- WebP
- AVIF

D’autres formats graphiques peuvent être acceptés lorsqu’ils sont correctement pris en charge par la pile de décodage, par exemple :

- GIF
- BMP
- TIFF
- XPM

La bibliothèque accepte uniquement des fichiers graphiques.

## 16. Suppression et annulation

La suppression déplace d’abord les assets vers une corbeille interne.

La restauration d’un asset rétablit son organisation et ses métadonnées.

L’application prend en charge l’annulation et le rétablissement des opérations pertinentes pendant la session.

La suppression logique d’un asset ne provoque pas immédiatement la destruction physique d’un objet devenu sans référence.

Un objet physique sans aucune référence devient orphelin. Les objets orphelins sont supprimés uniquement lors d’une opération de maintenance sûre.

## 17. Export et édition externe

L’export restitue le contenu préservé de l’asset.

L’ouverture d’un asset dans un éditeur externe utilise une copie de travail plutôt que d’exposer l’objet interne immuable aux modifications.

Le remplacement du contenu conserve l’UUID logique de l’asset ainsi que ses métadonnées d’organisation.

Le nouveau contenu produit un nouvel objet physique immuable identifié par son SHA-256. Le nom de fichier d’origine enregistré pour l’asset devient celui du nouveau contenu.

L’ancien objet physique peut alors devenir orphelin. Pigoune ne conserve pas d’historique permanent des versions du contenu.

Après le remplacement, les propriétés dérivées du fichier sont automatiquement recalculées :

- format ;
- dimensions ;
- poids ;
- SHA-256 ;
- miniature ;
- aperçus ;
- couleurs dominantes ;
- métadonnées intégrées ;
- index.

Si le nouveau SHA-256 existe déjà dans la bibliothèque, la déduplication physique reste transparente.

Pigoune ne s’enregistre pas comme visionneuse d’images généraliste du système.

## 18. Intégrité et récupération

Les objets stockés sont associés à leur identité SHA-256.

Pigoune fournit des mécanismes de vérification d’intégrité et de récupération.

SQLite constitue la source de vérité des métadonnées.

Un manifest de récupération indépendant est généré périodiquement. Il contient suffisamment d’informations essentielles pour permettre une reconstruction raisonnable de la bibliothèque en cas de corruption grave de SQLite.

Le manifest n’est pas utilisé comme seconde base active. Son format de sérialisation exact et sa stratégie d’atomicité relèvent de la conception technique ultérieure.

Après un crash, la bibliothèque doit revenir à son dernier état cohérent.

L’action « Optimiser la bibliothèque » vérifie les références avant de supprimer les objets orphelins. Elle peut également :

- vérifier la cohérence de la base ;
- nettoyer les caches obsolètes ;
- optimiser SQLite ;
- reconstruire les index si nécessaire ;
- mettre à jour le manifest de récupération.

Le contrôle complet d’intégrité reste une opération distincte de l’optimisation de la bibliothèque.

Les sauvegardes intégrées sont uniquement déclenchées manuellement et proposent deux modes :

- archive portable ;
- sauvegarde miroir.

Une sauvegarde correspond toujours à un état cohérent de la bibliothèque. Une sauvegarde à chaud cohérente est privilégiée. Si cette cohérence ne peut pas être garantie, Pigoune verrouille brièvement les écritures plutôt que de produire une sauvegarde incohérente.

Une sauvegarde peut être restaurée comme nouvelle bibliothèque ou remplacer explicitement la bibliothèque actuelle. Un remplacement exige d’abord la vérification de la sauvegarde et la création d’un point de sécurité de l’état actuel.

## 19. Architecture technique

L’application est développée en Rust.

Le cœur métier de la bibliothèque reste indépendant de GTK.

Les principales technologies prévues comprennent :

- Rust ;
- GTK 4 ;
- Libadwaita ;
- SQLite ;
- FTS5 ;
- Glycin ;
- Flatpak.

SQLite utilise des réglages privilégiant la durabilité sur le stockage local.

La recherche est abstraite afin que son implémentation puisse évoluer indépendamment du reste du cœur.

Les traitements en arrière-plan sont coordonnés par un gestionnaire central de tâches.

## 20. Périmètre de la version 1.0

La version 1.0 doit fournir une expérience complète et fiable pour un usage quotidien autour de :

- une bibliothèque principale autonome ;
- stockage adressé par contenu ;
- déduplication physique ;
- imports robustes ;
- principaux formats graphiques ;
- vues grille et liste ;
- collections hiérarchiques ;
- tags ;
- favoris ;
- recherche locale et filtres ;
- recherches enregistrées ;
- inspecteur ;
- noms d’affichage ;
- notes et métadonnées de source/licence ;
- familles de variantes ;
- aperçu rapide ;
- vue détail ;
- glisser-déposer ;
- workflows via le presse-papiers ;
- corbeille ;
- annulation/rétablissement ;
- remplacement du contenu ;
- export ;
- sauvegarde et restauration ;
- vérification d’intégrité ;
- récupération après crash ;
- données de récupération ;
- interface GNOME adaptative ;
- navigation clavier ;
- palette de commandes ;
- distribution Flatpak ;
- localisation française et anglaise.

La qualité prime sur le nombre de fonctionnalités.

## 21. Orientations après la 1.0

L’architecture doit permettre des évolutions futures telles que :

- détection visuelle des images similaires ;
- mode de comparaison avancé ;
- prise en charge complète des bibliothèques réseau/NAS ;
- plusieurs bibliothèques ;
- CLI de maintenance ;
- canaux de publication Stable/Beta.

Ces fonctionnalités ne doivent pas retarder une version 1.0 de qualité.

## 22. Principes UX

- Préférer les actions réversibles aux dialogues de confirmation inutiles.
- Ne jamais effectuer silencieusement une opération destructive.
- Ne pas exposer de fonctionnalités inachevées ou factices.
- Conserver une interface calme même avec de grandes bibliothèques.
- Maintenir les assets graphiques au centre de l’expérience visuelle.
- Ne pas transformer Pigoune en simple gestionnaire de fichiers avec de grandes vignettes.
- Les opérations lourdes ne doivent pas bloquer l’interface.
- Une action annoncée comme enregistrée doit être réellement durable.
- Protéger l’intégrité des données avant d’optimiser les performances.
- Garder la complexité d’implémentation invisible pendant l’utilisation normale.
