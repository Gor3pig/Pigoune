<p align="center">
  <img src="data/icons/pigoune-256.png" alt="Icône Pigoune" width="160" height="160">
</p>

<h1 align="center">Pigoune</h1>

<p align="center">
  <strong>Plateforme d’Icônes et Graphismes — Organisée, Unifiée, Native et Élégante.</strong>
</p>

<p align="center">
  Français · <a href="README.en.md">English</a>
</p>

Pigoune est une bibliothèque personnelle d’assets graphiques conçue nativement pour Linux et GNOME. Elle vise à offrir un espace fiable et visuel pour organiser, conserver et retrouver des icônes, logos, illustrations et autres ressources graphiques, entièrement en local et hors ligne.

## État du projet

Pigoune est en phase initiale de développement. Les fondations Rust, GTK 4, Libadwaita et Flatpak sont en place, mais aucune version stable n’est encore publiée et l’interface produit reste à construire.

## Pourquoi Pigoune ?

Les ressources graphiques finissent souvent dispersées dans des arborescences difficiles à parcourir ou dépendantes de leur emplacement d’origine. Pigoune veut proposer une bibliothèque dédiée, native GNOME, où chaque asset importé devient indépendant de son fichier source.

## Principes clés

- fonctionnement local et hors ligne, sans compte, cloud, télémétrie ni analytics ;
- préservation bit-à-bit du contenu binaire importé ;
- identification du contenu physique par SHA-256 et identité propre pour chaque asset logique ;
- bibliothèques autonomes et portables ;
- priorité donnée à l’intégrité, à la durabilité et à la récupération des données.

## Objectifs principaux de la 1.0

La première version stable vise une base cohérente pour importer et préserver les assets, les organiser et les rechercher, produire des aperçus, les exporter et protéger la bibliothèque grâce à des mécanismes d’intégrité, de sauvegarde et de récupération.

## Technologies

| Technologie | Rôle prévu |
| --- | --- |
| Rust | Application et cœur métier |
| GTK 4 et Libadwaita | Interface native GNOME |
| SQLite | Métadonnées de la bibliothèque |
| Glycin | Chargement et décodage des images |
| Flatpak | Distribution et isolation |

## Architecture

Le workspace sépare `pigoune-core`, qui porte le domaine sans dépendance graphique, de `pigoune-app`, le frontend GNOME. Les échanges entre les deux couches resteront explicites et testables.

## Documentation

- [Spécification produit — français, version canonique](docs/product/specification.md)
- [Product specification — English translation](docs/product/specification.en.md)

## Licence

Pigoune est distribué sous licence [GNU General Public License version 3 ou ultérieure](LICENSE).
