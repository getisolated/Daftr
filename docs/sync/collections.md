# Synchronisation des collections entre appareils

Statut : **plan validé**. Prolonge la synchronisation LAN des notes ([plan.md](plan.md)).

## 1. Décisions prises

| # | Sujet | Décision |
|---|-------|----------|
| 1 | Sens | Toujours miroir : les deux PC lisent et écrivent. | ✅ |
| 2 | Fichiers | **Seulement les `.md`.** Jamais les autres fichiers du dossier, ni pour les copier ni pour les supprimer. Mêmes exclusions que l'app (`node_modules`, `bin`, `obj`, `build`, `dist`, `target`…, voir `DEFAULT_COLLECTION_EXCLUDES`). | ✅ |
| 3 | Suppressions | Propagées, mais le fichier part à la **corbeille du système** sur l'autre PC (récupérable), sans confirmation. | ✅ |
| 4 | Destination | Réglage « Dossier des collections reçues » (par PC, non synchronisé, défaut `Documents/Daftr/`), `<racine>/<nom de la collection>` pré-rempli, modifiable à chaque réception. | ☐ |
| 5 | Dossier existant | « Fusionner » possible : identiques ignorés, absents copiés, différents gardés en double (pas d'historique commun au premier échange). | ☐ |
| 6 | Dossier avec `.git` | Simple avertissement, pas d'interdiction. | ☐ |

| 7 | Appareils | Une collection ne se partage qu'avec les appareils qui **partagent déjà le profil** (la liste des collections fait partie des données du profil). |
| 8 | Taille | 10 Mio maximum par fichier ; un fichier plus gros est ignoré avec un message. |
| 9 | Branche | Même branche et même PR (#90) que la synchronisation des notes : c'est la même fonctionnalité. |

## 2. Parcours utilisateur (70 %)

### 2.1 Activer le partage d'une collection (PC émetteur)

Trois entrées, le même assistant :

- clic droit sur la collection → **« Synchroniser avec mes appareils… »** ;
- pastille de synchro sur la ligne de la collection (clic = ouvre l'assistant ou le détail) ;
- popup du profil → « Appareils et synchronisations » → section **« Collections partagées »** → « Ajouter une collection ».

Assistant en une seule fenêtre :

```
┌ Synchroniser « WMS.BusinessCentral » ───────────────┐
│ Avec quels appareils ?                              │
│  ☑ PC-Portable   (portable)                         │
│  ☐ PC-Bureau     (fixe)                             │
│                                                     │
│ ⓘ 142 fichiers .md (3,2 Mo) seront partagés.        │
│   Les autres fichiers du dossier ne sont pas        │
│   touchés.                                          │
│ ⚠ Ce dossier est géré par git : les .md peuvent     │
│   aussi être modifiés par git.        (si .git)     │
│                                                     │
│                      [ Synchroniser ]  [ Annuler ]  │
└─────────────────────────────────────────────────────┘
```

Si aucun appareil ne partage le profil : « Liez d'abord un appareil » avec un bouton vers la vue QR du popup profil.

### 2.2 Recevoir une collection (PC récepteur)

Un toast « PC-Portable propose la collection WMS.BusinessCentral (142 fichiers) » ouvre :

```
┌ Collection proposée par PC-Portable ────────────────┐
│ WMS.BusinessCentral · 142 fichiers · 3,2 Mo         │
│                                                     │
│ Dossier de destination                              │
│ ┌─────────────────────────────────────┐             │
│ │ D:\Daftr\WMS.BusinessCentral        │ [Modifier…] │
│ └─────────────────────────────────────┘             │
│ ⚠ Ce dossier existe déjà (87 .md).                  │
│   ◉ Fusionner avec son contenu                      │
│   ○ Choisir un autre dossier                        │
│                                                     │
│          [ Accepter ]  [ Ignorer ]  [ Plus tard ]   │
└─────────────────────────────────────────────────────┘
```

- Rien n'est écrit tant que l'utilisateur n'a pas cliqué **Accepter**.
- **Ignorer** refuse cette proposition, sans bloquer les suivantes ; **Plus tard** garde la proposition dans la liste des offres.
- L'avertissement « dossier existant » n'apparaît que si le dossier contient déjà des fichiers ; la fusion est alors préselectionnée.
- Si la proposition est acceptée, la collection apparaît dans la sidebar avec une barre de progression « 37 / 142 », puis la pastille « à jour ».

### 2.3 États dans la sidebar

| Pastille | Sens | Survol |
|----------|------|--------|
| ⟳ discret | à jour | « Synchronisée avec PC-Portable, il y a 2 min » |
| ⟳ animé | en cours | « 37 / 142 fichiers » |
| ⚠ | conflits à résoudre | « 2 fichiers en conflit » |
| ⏸ | l'autre PC est absent | « En attente de PC-Portable » |
| ✕ | dossier introuvable | « Dossier introuvable » + « Relier à un autre dossier » |

### 2.4 Conflits

- Un fichier modifié des deux côtés : fusion à 3 voies si possible (mêmes règles que les notes), sinon **copie** `nom (conflit PC-Portable).md` à côté de l'original. Jamais de perte.
- Les copies en conflit apparaissent dans l'arbre avec un badge ⚠. En haut du fichier, un bandeau : **« Garder ma version » · « Garder l'autre » · « Comparer »**. Choisir supprime la copie (corbeille).
- Une modification gagne toujours sur une suppression : si A supprime un fichier que B vient de modifier, il est rétabli sur A.

### 2.5 Arrêter et cas limites

- **« Arrêter de synchroniser cette collection »** (clic droit, ou croix dans la section du popup) : les fichiers restent des deux côtés, le lien est coupé. Un choix « supprimer ma copie » n'est pas proposé (trop risqué).
- **Retirer l'appareil** (Paramètres → Synchronisation) coupe aussi ses collections, comme pour les profils.
- Dossier renommé ou déplacé sur un PC : « Dossier introuvable », « Relier à un autre dossier ».
- Fichier supprimé par erreur : récupérable dans la corbeille du système.

### 2.6 Réglage

Paramètres → Synchronisation → **« Dossier des collections reçues »** : chemin + « Parcourir… ».

## 3. Technique (30 %)

**Principe.** Une collection n'a pas de contenu en base : on synchronise des fichiers. On garde les canaux existants (session chiffrée, appareils de confiance, espace du profil) et on ajoute une couche « fichiers ».

- **Identité.** Chaque collection partagée reçoit un `sync_id` (uuid) stocké avec elle ; le chemin reste local à chaque PC. Le profil porte la liste, l'espace du profil décide des appareils.
- **État par fichier.** Chemin relatif (séparateur `/`), hash du contenu, et dernière version synchronisée par pair (la « base »). La base permet de savoir qui a changé : local ≠ base et distant = base → on envoie ; l'inverse → on reçoit ; les deux ≠ base → conflit. Le texte de la base est gardé pour la fusion à 3 voies (réutilise `text_merge`).
- **Détection des changements.** Le watcher existant des collections déclenche un recalcul des hash du fichier concerné ; un scan complet au démarrage et à la connexion d'un pair rattrape le reste.
- **Protocole.** Nouveaux messages dans la session existante : manifeste (chemin + hash + taille), demande de fichier, morceaux de fichier (~256 Kio), accusé. La version de protocole reste compatible (messages inconnus ignorés par les anciennes versions).
- **Écriture sur disque.** Fichier temporaire puis renommage atomique ; on mémorise le hash écrit pour que le watcher n'y voie pas une modification locale (pas de boucle). Suppression par la crate `trash` (déjà utilisée).
- **Sécurité.** Chemins distants validés (pas de `..`, pas de chemin absolu, noms réservés Windows refusés, liens symboliques non suivis, via `resolve_inside`). Rien n'est jamais écrit hors du dossier de la collection.
- **Limites.** Reprend `MAX_SCANNED_FILES` (20 000) et `MAX_SCAN_DEPTH` ; taille maximale par fichier (10 Mio). Casse des noms : Windows insensible, Linux sensible ; deux fichiers qui ne diffèrent que par la casse sont signalés en conflit.
- **Renommage.** Vu comme suppression + création en v1 (le contenu étant identique, le transfert reste rapide ; détection par hash possible plus tard).

## 4. Lots

| Lot | Contenu | Livrable testable | Fait |
|-----|---------|-------------------|------|
| 0 | Données : `sync_id` des collections, table d'état des fichiers (hash, base), réglage « Dossier des collections reçues ». | Tests de migration et de stockage. | ✅ |
| 1 | Cœur pur (domain/app) : calcul de diff entre manifestes, décision envoyer / recevoir / conflit / suppression, noms de copies de conflit, validation des chemins. | Tests unitaires et `proptest` (convergence, jamais de perte). | ✅ |
| 2 | Protocole et moteur : messages, transfert par morceaux, écriture atomique, corbeille, anti-boucle, reprise après coupure. | Tests d'intégration deux moteurs en mémoire. | ✅ |
| 3 | Intégration desktop : watcher → moteur, offres de collection, commandes IPC, événements de progression. | Tests de commandes ; recette sur deux dossiers locaux. | ✅ |
| 4 | Interface : menu contextuel, assistant, fenêtre de proposition, pastilles, section dans le popup profil, réglage de la racine. | Tests vitest des utilitaires ; recette visuelle. | ☐ |
| 5 | Conflits : badge, bandeau d'actions, « Comparer », états « introuvable » / « en attente ». | Tests vitest ; recette. | ☐ |
| 6 | Documentation (plan.md, README) et recette sur deux PC réels. | — | ☐ |

Tout va dans la branche `NF/feat/sync-lan-261123`, PR #90 vers `recipe`.

## 5. Hors périmètre (pour l'instant)

Images et pièces jointes, fichiers non-markdown, partage avec d'autres personnes, détection de renommage, synchronisation en temps réel pendant la frappe (les fichiers se synchronisent à l'enregistrement).
