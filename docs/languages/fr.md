# PDF Editor Offline

**Un éditeur PDF hors ligne en Rust. Vos fichiers restent sur votre ordinateur.**

[English](../../README.md) · [Français](fr.md) · [العربية](ar.md) · [Español](es.md) · [Português](pt-BR.md) · [Deutsch](de.md) · [简体中文](zh-CN.md) · [日本語](ja.md) · [한국어](ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml) · [MIT](../../LICENSE)

Ouvrez un PDF, trouvez la page voulue, organisez le document et enregistrez une nouvelle copie. PDF Editor Offline est une application de bureau et un outil en ligne de commande gratuits et libres. Aucun compte, abonnement ou téléversement de document n'est nécessaire.

Il s'agit de l'aperçu natif Rust. Compilez l'application de bureau depuis les sources pour la tester. macOS, Windows et Linux sont les plateformes visées ; les interactions de bureau ont surtout été vérifiées sur macOS. Les installateurs de bureau et l'édition PDF complète restent en développement. La [préversion CLI native 4.0.0-preview.1](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.1) est disponible ; les [versions de bureau archivées](https://github.com/OthmaneBlial/pdf-editor-offline/releases) correspondent à des versions antérieures.

![Vraie fenêtre native sur macOS : recherche locale, page PDF et propriétés du document](../../screenshots/native/editorial-light-2026-10-12.png)

## Ce qui fonctionne

| Fonction | Disponibilité |
| --- | --- |
| Lire et chercher | Onglets, zoom, miniatures visibles, texte, recherche littérale et thèmes clair/sombre ; informations, extraction et comparaison limitée en CLI. |
| Organiser | Déplacement, inversion, rotation, pages vierges, recadrage et annuler/rétablir dans l'application ; extraction/suppression protégées et fusion de catalogues simples en CLI. |
| Propriétés | Lecture et modification de six champs standard : Title, Author, Subject, Keywords, Creator et Producer, avec Unicode. |
| Images et commentaires | PDF à partir de PNG/JPEG pris en charge ; export d'une page en PNG et ajout/liste/modification/suppression protégées de notes Text en CLI. |
| Inspecter et enregistrer | Inventaire en lecture seule des marqueurs d'actions, pièces jointes, formulaires et calques en CLI ; compression des flux sans perte et copies vérifiées sans écrasement. |

Les fonctions d'assemblage, de sous-ensembles de pages, de notes et d'images ont des restrictions explicites. Les structures non prises en charge sont refusées ; voir le [guide CLI](../../apps/cli/README.md). La compression ne garantit pas un fichier plus petit. Le recadrage masque du contenu ; retirer des métadonnées ou des notes peut laisser d'anciennes valeurs. Ce n'est pas un effacement sécurisé. OCR, remplissage de formulaires, signature numérique, modification libre du texte et conversion générale ne sont pas disponibles. Le rendu peut omettre du contenu non pris en charge. Vérifiez les fichiers exportés. Les documents signés et ceux déchiffrés avec succès restent en lecture seule.

L'interface est actuellement en anglais. Ces traductions ne signifient pas que l'application est localisée ou qu'elle gère entièrement la mise en page de droite à gauche. Les polices hors ligne incluent des glyphes chinois, japonais et coréens testés, sans couvrir tous les systèmes d'écriture.

## Installer la CLI native (facultatif)

Pour l'installation npm et son lanceur, utilisez Node.js 22+. Le binaire CLI autonome n'a pas besoin de Node.js. L'application de bureau se compile toujours depuis les sources Rust.

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --help
```

## Compiler et lancer

Installez Rust 1.95+ et les bibliothèques natives de votre plateforme. Le premier téléchargement des dépendances nécessite une connexion. Pour Linux, consultez les [paquets de la CI native](../../.github/workflows/rust.yml).

```sh
git clone https://github.com/OthmaneBlial/pdf-editor-offline.git
cd pdf-editor-offline
cargo build --release --locked -p pdf-editor-offline-native -p pdf-editor-offline-cli
./target/release/pdf-editor-offline-native
```

```sh
./target/release/pdf-editor-offline info examples/sample_pdfs/demo-basic.pdf --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --json
./target/release/pdf-editor-offline capabilities --json
```

Sur Windows, lancez `target\release\pdf-editor-offline-native.exe`. Les pages commencent à 1. `--dry-run` valide sans écrire ; choisissez un nouveau chemin de sortie, les fichiers et liens symboliques existants ne sont jamais écrasés.

## Local et vérifiable

Les opérations PDF s'exécutent sur votre machine, sans service de rendu ou conversion à distance. Cela ne prouve ni l'innocuité de chaque PDF ni la suppression de toutes les données cachées. Les [preuves CLI](../RUST_BENCHMARKS.md) présentent 44 contrôles d'opérations avec qpdf/Poppler, des temps de processus et des empreintes, dont un PDF synthétique de 1 000 pages. Elles ne garantissent pas la fidélité de tous les documents, la réactivité du bureau ou un gain de vitesse universel.

Contributions utiles : cas reproductibles avec PDF synthétiques, comparaisons de rendu, tests d'accessibilité, corrections de traduction et petites modifications Rust. N'incluez pas de documents privés dans les rapports publics.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Status](../RUST_STATUS.md) · [Desktop](../../apps/desktop/README.md) · [CLI](../../apps/cli/README.md) · [Security](../SECURITY_INSPECTION_RUST.md) · [Screenshots](../../screenshots/native/)

Si un éditeur PDF local vous est utile, ajoutez une étoile au dépôt pour aider d'autres personnes à le trouver.

Licence MIT ; les polices et dépendances conservent leurs propres notices.
