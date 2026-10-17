<div align="center">

<img src="data/icons/pigoune-256x256.png" alt="Pigoune" width="160" height="160">

# Pigoune

**P**lateforme d'**I**cônes et **G**raphismes **O**rganisée, **U**nifiée, **N**ative et **É**légante

*Tous vos éléments graphiques, rangés et à portée de main.*

[![Licence : GPL v3](https://img.shields.io/badge/licence-GPL--3.0-3584e4?style=for-the-badge)](COPYING)
[![GNOME](https://img.shields.io/badge/GNOME-51-4a86cf?style=for-the-badge&logo=gnome&logoColor=white)](https://www.gnome.org)
[![Rust](https://img.shields.io/badge/Rust-GTK4%20%C2%B7%20libadwaita-e66100?style=for-the-badge&logo=rust&logoColor=white)](https://gtk-rs.org)
[![Dernière version](https://img.shields.io/github/v/release/Gor3pig/Pigoune?style=for-the-badge&label=version&color=9141ac&logo=flatpak&logoColor=white)](https://github.com/Gor3pig/Pigoune/releases/latest)

[English](README.md) · **Français**

[Site web](https://gor3pig.github.io/Pigoune/fr/) · [Guide d'utilisation](help/fr/README.md) · [Fonctionnalités](#fonctionnalités) · [Principes](#principes) · [Installation](#installation) · [Contribuer](#contribuer)

<img src="data/screenshots/01.png" alt="La fenêtre principale de Pigoune" width="860">

</div>

---

## Pigoune, c'est quoi ?

Des icônes dans `Téléchargements`, des logos dans un vieux dossier de projet, un SVG perdu
quelque part sur le bureau… **Pigoune rassemble tout ça dans une seule bibliothèque**, bien
rangée et agréable à parcourir. Vos fichiers d'origine ne sont jamais touchés.

## Fonctionnalités

<table>
<tr>
<td width="33%" valign="top">

**Déposer**<br>Fichiers et dossiers, tous les formats courants, doublons repérés.

</td>
<td width="33%" valign="top">

**Organiser**<br>Collections, collections dynamiques, tags imbriqués, favoris.

</td>
<td width="33%" valign="top">

**Retrouver**<br>Recherche et filtres instantanés, même parmi des milliers.

</td>
</tr>
<tr>
<td width="33%" valign="top">

**Réutiliser**<br>Glisser, copier, exporter, ou en faire un fond d'écran.

</td>
<td width="33%" valign="top">

**Annoter**<br>Flèches, texte et flou ; l'original n'est jamais touché.

</td>
<td width="33%" valign="top">

**Admirer**<br>Aperçu avec zoom, animations au survol, corbeille et annulation.

</td>
</tr>
</table>

<p align="center"><b><a href="help/fr/README.md">Lire le guide d'utilisation →</a></b><br>Tout ce que Pigoune sait faire, pas à pas.</p>

## Principes

**Originaux jamais modifiés** · **100 % local, sans compte** · **Bibliothèques portables** · **Pensé pour GNOME**

## Installation

### 1. Préparer Flatpak

Fedora, Linux Mint, Pop!_OS et beaucoup d'autres distributions l'intègrent déjà. Sur Ubuntu et
quelques autres, suivez d'abord le [guide officiel pour votre distribution](https://flathub.org/setup),
puis redémarrez votre session.

### 2. Installer Pigoune

Ouvrez [pigoune.flatpakref](https://gor3pig.github.io/Pigoune/pigoune.flatpakref) avec Logiciels,
ou lancez cette commande :

```sh
flatpak install --user https://gor3pig.github.io/Pigoune/pigoune.flatpakref
```

### 3. Rester à jour

Pigoune se met à jour comme vos autres applications, et chaque mise à jour est signée. Le fichier
`.flatpak` de chaque version est aussi joint à sa
[version sur GitHub](https://github.com/Gor3pig/Pigoune/releases), et vous pouvez compiler Pigoune
vous-même en suivant [CONTRIBUTING.md](CONTRIBUTING.md).

Pigoune fonctionne sous toute distribution Linux qui accepte Flatpak, sur un PC 64 bits Intel ou
AMD. Il n'existe pas de version pour Windows, macOS ni pour les appareils ARM.

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

Les icônes des collections viennent de [Phosphor Icons](https://phosphoricons.com), distribuées
sous [licence MIT](crates/pigoune-app/icons/LICENSE-phosphor).

