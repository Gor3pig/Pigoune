<div align="center">

<img src="data/icons/pigoune-256x256.png" alt="Pigoune" width="160" height="160">

# Pigoune

**P**lateforme d'**I**cônes et **G**raphismes **O**rganisée, **U**nifiée, **N**ative et **É**légante

*Toutes vos ressources graphiques, rangées et à portée de main.*

[![Licence : GPL v3](https://img.shields.io/badge/licence-GPL--3.0-3584e4?style=for-the-badge)](COPYING)
[![GNOME](https://img.shields.io/badge/GNOME-51-4a86cf?style=for-the-badge&logo=gnome&logoColor=white)](https://www.gnome.org)
[![Rust](https://img.shields.io/badge/Rust-GTK4%20%C2%B7%20libadwaita-e66100?style=for-the-badge&logo=rust&logoColor=white)](https://gtk-rs.org)
[![Dernière version](https://img.shields.io/github/v/release/Gor3pig/Pigoune?style=for-the-badge&label=version&color=9141ac&logo=flatpak&logoColor=white)](https://github.com/Gor3pig/Pigoune/releases/latest)

[English](README.md) · **Français**

[Fonctionnalités](#fonctionnalités) · [Principes](#principes) · [Installation](#installation) · [Guide](#guide-dutilisation) · [Contribuer](#contribuer)

<img src="data/screenshots/01.png" alt="La fenêtre principale de Pigoune" width="860">

</div>

---

> [!NOTE]
> **Pigoune est disponible.** [Téléchargez le paquet Flatpak de la dernière version](https://github.com/Gor3pig/Pigoune/releases/latest)
> pour l'essayer.

## Pigoune, c'est quoi ?

Des icônes dans `Téléchargements`, des logos dans un vieux dossier de projet, un SVG perdu
quelque part sur le bureau… **Pigoune rassemble tout ça dans une seule bibliothèque**, bien
rangée et agréable à parcourir.

Déposez-y vos ressources graphiques : Pigoune en garde une copie en lieu sûr (vos fichiers
d'origine ne sont jamais touchés), vous aide à les organiser et vous les rend d'un simple
glisser-déposer quand vous en avez besoin.

## Fonctionnalités

<table>
<tr>
<td width="50%" valign="top">

### Déposer
Glissez des fichiers ou des dossiers entiers : SVG, PNG, JPEG, WebP, AVIF, JPEG XL, GIF, TIFF,
BMP et ICO. Chaque image est vérifiée avant d'entrer, et les doublons sont reconnus
automatiquement.

</td>
<td width="50%" valign="top">

### Organiser
Collections imbriquées, tags, favoris, ainsi qu'une note, une source, une licence et un auteur
pour chaque ressource.

</td>
</tr>
<tr>
<td width="50%" valign="top">

### Retrouver
Recherche instantanée et filtres par type ou par favori, même parmi des milliers de
ressources. Un clic sur une collection ou un tag vous montre où se trouve la ressource.

</td>
<td width="50%" valign="top">

### Réutiliser
Glissez une ressource vers n'importe quelle application, ouvrez-la avec une autre, copiez-la
dans le presse-papiers ou exportez-la dans un dossier.

</td>
</tr>
<tr>
<td width="50%" valign="top">

### Admirer
Grille de vignettes à taille réglable, animations (GIF, PNG, WebP) au survol, et aperçu détaillé
avec zoom d'une simple pression sur <kbd>Espace</kbd>.

</td>
<td width="50%" valign="top">

### Se rassurer
Corbeille et annulation avec <kbd>Ctrl</kbd>+<kbd>Z</kbd> : une erreur se rattrape toujours.

</td>
</tr>
</table>

## Principes

- **Vos originaux ne sont jamais modifiés** : Pigoune travaille sur ses propres copies.
- **100 % local** : pas de compte, pas de cloud, pas de télémétrie.
- **Une bibliothèque portable** : un simple dossier que vous placez où vous voulez, et que vous
  pouvez emporter sur un autre ordinateur.
- **Pensé pour GNOME** : une application native et soignée, en mode clair comme sombre,
  utilisable au clavier et avec un lecteur d'écran, qui s'adapte aux petites fenêtres.

## Installation

### 1. Préparer Flatpak

Pigoune est distribué au format Flatpak. Fedora, Linux Mint, Pop!_OS et beaucoup d'autres
distributions l'intègrent déjà. Sur Ubuntu et quelques autres, installez-le d'abord en suivant le
[guide officiel pour votre distribution](https://flathub.org/setup), puis redémarrez votre
session.

### 2. Installer Pigoune

Téléchargez le fichier `.flatpak` de la [dernière version](https://github.com/Gor3pig/Pigoune/releases/latest),
puis ouvrez-le avec Logiciels, ou lancez cette commande depuis le dossier où vous l'avez
enregistré :

```sh
flatpak install --user pigoune-*.flatpak
```

L'environnement GNOME dont Pigoune a besoin est téléchargé depuis Flathub la première fois.

### 3. Mettre à jour Pigoune

Le paquet ne se met pas encore à jour tout seul. Quand une nouvelle version sort, téléchargez son
fichier `.flatpak` et installez-le par-dessus l'actuel, avec Logiciels ou avec cette commande :

```sh
flatpak install --user --reinstall pigoune-*.flatpak
```

Si le dossier contient plusieurs fichiers Pigoune, tapez plutôt le nom complet du plus récent.
Vos bibliothèques et vos réglages sont conservés. Vous pouvez aussi compiler Pigoune vous-même
en suivant [CONTRIBUTING.md](CONTRIBUTING.md).

## Guide d'utilisation

Le [guide d'utilisation](help/fr/README.md) présente pas à pas tout ce que Pigoune sait faire.
Dans l'application, il s'ouvre aussi depuis le menu principal ou avec <kbd>F1</kbd>.

## Contribuer

Pigoune est un logiciel libre, et toute aide est la bienvenue : signaler un bug, proposer une
idée, traduire ou améliorer le code. Le guide [CONTRIBUTING.md](CONTRIBUTING.md) (en anglais)
explique comment compiler Pigoune et proposer une modification,
[ARCHITECTURE.md](ARCHITECTURE.md) présente l'organisation du code et
[po/README.md](po/README.md) explique comment le traduire.

## Licence

Pigoune est un logiciel libre distribué sous licence [GNU GPL v3 ou ultérieure](COPYING).

