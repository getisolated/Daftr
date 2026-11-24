# Refonte de l'ergonomie de la synchronisation

Statut : **plan validé** (retour de recette du 2026-11-24 sur deux PC réels : le moteur fonctionne, l'ergonomie est à refaire). Complète [plan.md](plan.md) et [collections.md](collections.md) ; là où ce document les contredit, il l'emporte.

## 1. Constat

- Le moteur synchronise (notes, collection), mais l'appairage mélangeait « lier deux appareils » et « choisir quoi partager » : l'utilisateur a appairé deux fois les mêmes appareils et a oublié de basculer sur le profil reçu.
- Une proposition de profil demandait « nouveau profil ou profil existant » en deux listes : trop de va-et-vient entre les deux appareils.

## 2. Principe

| Étape | Fréquence | Qui décide | Effet |
|-------|-----------|------------|-------|
| **Appairer** | une seule fois | les deux appareils (code ou QR) | lien de confiance entre deux appareils du même « parc ». Aucun profil à choisir. |
| **Partager un profil** | à volonté | la source (A) | B reçoit **sans confirmation** un clone du profil (même nom, même icône). Toast sur B : « Profil “X” reçu depuis A » + bouton **Basculer**. |
| **Partager une collection** | à volonté | la source (A) | B reçoit une demande **à configurer** dans l'écran de synchronisation ; rien n'est écrit sur son disque avant son « Activer ». |

Décisions :

1. L'appairage est symétrique ; ensuite chaque appareil peut partager vers l'autre.
2. Un profil protégé par mot de passe maître se partage quand même : le mot de passe n'est **pas** transmis, le clone arrive sans mot de passe, et un avertissement le dit à côté de la case sur A.
3. « Fusionner avec un profil existant » disparaît : un clone arrive toujours comme nouveau profil, même si B a déjà un profil du même nom. Le profil par défaut sans nom de A arrive sous le nom affiché « Profil par défaut ».
4. Une collection se partage avec n'importe quel appareil appairé, **sans** que son profil soit synchronisé (annule la décision 7 de collections.md). Le profil cible sur B est celui de la source si ce profil est synchronisé vers B, sinon B le choisit.
5. B garde le contrôle de son disque : une collection reçue reste « à configurer » tant qu'il n'a pas validé son profil cible et son dossier.

## 3. Écran « Synchronisation »

Un écran dédié (le popup du profil perd sa sous-vue « Appareils et synchronisations »). Ouvert depuis la ligne d'état de la barre latérale (« À jour (1 appareil) »), le menu du profil et Paramètres → Synchronisation. Deux colonnes : **partage et appareils** à gauche, **lier un appareil** à droite.

```
┌ Synchronisation ──────────────────────────────────────────────────────────┐
│ PARTAGE            [Profil : Perso ▾]   │ LIER UN APPAREIL                 │
│                       bubulle-book      │ ┌──────────┐                     │
│ ☑ Profil « Perso »        ☑             │ │    QR    │                     │
│   ⚠ mot de passe non transmis           │ └──────────┘                     │
│ Collections de ce profil                │      182 847                     │
│ ☑ LAURE                   ☑             │   Valable encore 4:34            │
│ ☐ WMS                     ☐             │ Sur l'autre appareil, choisissez │
│ ┌╌ GB.backlog  · À configurer ╌╌╌╌╌╌╌┐  │ « Se lier à un appareil »…       │
│ ╎ Reçue de bubulle-book              ╎  │      Saisir un code              │
│ ╎ Profil  [Perso ▾]                  ╎  │                                  │
│ ╎ Dossier D:\Daftr\GB.backlog [Modif.]╎  │                                  │
│ ╎              [Activer] [Ignorer]   ╎  │                                  │
│ └╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘  │                                  │
│ ─────────────────────────────────────── │                                  │
│ APPAREILS                               │                                  │
│ 💻 bubulle-book  8A2F-4145   [Retirer]  │                                  │
└───────────────────────────────────────────────────────────────────────────┘
```

- **Matrice** profils / collections × appareils : une colonne de cases par appareil (une seule tant qu'il n'y en a qu'un). Le sélecteur de profil change les collections affichées.
- **Les partages en attente sont des partages comme les autres**, mêlés à la liste : même ligne, avec un style atténué (bordure en pointillé, pastille « À configurer ») qui dit qu'il manque une configuration. Un clic déplie le formulaire en ligne : profil cible, dossier (affiché, modifiable, avertissement « dossier existant (N .md) » + fusion), Activer / Ignorer.
- Une demande dont le profil source n'est pas synchronisé vers B (profil cible à choisir) apparaît dans le groupe « Sans profil » en bas de la liste, visible quel que soit le profil sélectionné.
- Le clic droit sur une collection reste : « Synchroniser avec… », mêmes cases dans un petit menu.
- La ligne d'état de la barre latérale porte un badge quand des partages attendent une configuration.
- Le bloc de droite n'affiche que l'appairage : QR et code d'office, minuteur, phrase, lien « Saisir un code ».

## 4. Saisie du code

Remplace le bloc de droite, sans listes autour.

- Code en grand, centré, six cases de type OTP avec un trait de 2 px dessous.
- Passage automatique à la case suivante ; retour arrière, flèches, **coller** (6 chiffres d'un coup, espaces ignorés) et **couper** gérés.
- Champ d'adresse IP redessiné (libellé, aide, placeholder), optionnel.
- Boutons Lier / Annuler redessinés ; Lier actif dès que le code est complet.

## 5. Technique

- **Appairage sans profil** : il ne crée plus d'espace de synchronisation. Le moteur ne réclame plus de profils au démarrage du mode appairage.
- **Partage de profil automatique** : cocher un profil pour un appareil crée l'espace chez A et l'offre à B ; B l'accepte d'office en créant un nouveau profil (nom, icône) puis démarre la synchro initiale. L'ancienne réponse « fusionner / ignorer » et son dialogue sont retirés. Décocher = « Arrêter de synchroniser ce profil avec X ».
- **Collections liées à l'appareil** : le partage ne passe plus par l'espace du profil mais par la confiance de l'appareil ; l'offre mémorise le profil source (identifiant, nom). Chez B, l'offre est une entrée « à configurer » ; **Activer** crée la collection dans le profil choisi (profil éventuellement inactif) et démarre l'échange.
- **Événements et commandes** : nouvelle commande de partage par appareil, activation d'une offre avec profil cible, événement « profil reçu » pour le toast. Les commandes obsolètes sont supprimées.
- **Compatibilité** : le protocole reste en version 1 ; les nouveaux champs suivent la règle des octets de fin et du bit `features` déjà utilisés.

## 6. Lots

| Lot | Contenu | Fait |
|-----|---------|------|
| R0 | Backend : appairage sans profil, partage de profil par appareil, acceptation automatique et clonage, événement « profil reçu ». | ☑ |
| R1 | Backend : collections liées à l'appareil, offres « à configurer », activation avec profil cible. | ☑ |
| R2 | Front : écran « Synchronisation » (partage, appareils, partages en attente mêlés, lier un appareil), retrait de l'ancienne sous-vue, toast « Basculer ». | ☐ |
| R3 | Front : saisie du code (OTP, coller / couper) et formulaire d'adresse. | ☐ |
| R4 | Documentation (plan.md, README) et nouvelle recette sur deux PC. | ☐ |

Tout va dans la branche `NF/feat/sync-lan-261123`, PR #90 vers `recipe`.
