# Synchronisation multi-appareils — plan

> Rédigé le 2026-10-29. Décisions prises : serveur maison (Rust) sur un VPS OVH, CRDT,
> pas de chiffrement de bout en bout, connexion GitHub / Google (Microsoft et Apple plus tard).

## 1. Principes

- **Local-first.** SQLite reste la source de vérité locale et l'app marche entièrement hors ligne.
  Le serveur est un relais entre appareils et une sauvegarde. S'il tombe, personne ne perd rien :
  la sync s'arrête, c'est tout.
- **Le serveur est interchangeable.** Son URL est un réglage (valeur par défaut dans l'app).
  N'importe qui peut héberger le sien ; reprendre le projet = copier un fichier SQLite et
  enregistrer ses propres apps OAuth (voir §7).
- **Aucun secret dans l'app.** Les secrets GitHub / Google vivent sur le serveur. L'app ne fait que
  du PKCE contre *ton* serveur, qui joue le rôle de courtier OAuth.
- **Un document CRDT par note + un document « espace » pour les dossiers.** Les mises à jour sont
  petites, la fusion est caractère par caractère, deux PC ouverts en même temps convergent.

## 2. Choix de la bibliothèque CRDT : `yrs`

| | `yrs` (Y-CRDT, port Rust de Yjs) | `loro` | `automerge` |
|---|---|---|---|
| Maturité / déploiements | Très large (Yjs partout) | 1.0 fin 2024, jeune | Mûr, plus lent historiquement |
| Liaison CodeMirror 6 | `y-codemirror.next`, officielle | `loro-codemirror`, récente | Non officielle |
| Format binaire compatible JS | Oui (Yjs ↔ yrs) | Oui (wasm) | Oui (wasm) |
| Arbre de dossiers | `Y.Map` + règle de réparation des cycles | `LoroTree` natif (déplacements sûrs) | Pas de tree |
| Serveur | Un relais d'updates binaires, protocole y-sync connu (`y-sweet` existe en Rust) | À écrire | `automerge-repo` |

Choix : **`yrs`**. Raison principale : la phase 5 (curseurs partagés dans l'éditeur) se fait avec
une liaison CodeMirror existante et le même format binaire côté webview et côté Rust.
Seul point faible : les dossiers. Deux déplacements concurrents peuvent créer un cycle ; règle
déterministe après fusion : le dossier au plus petit `uuid` du cycle est rattaché à la racine.

## 3. Architecture cible

```
┌──────────── Daftr (client) ─────────────┐        ┌───────── VPS OVH ──────────┐
│ webview Angular                         │        │ Caddy (TLS Let's Encrypt)  │
│   └─ CodeMirror (phase 5 : y-codemirror)│  HTTPS │   └─ daftr-server (axum)   │
│ Rust                                    │  + WS  │        ├─ auth : courtier   │
│   ├─ adapter-sqlite : lignes + blob CRDT│◄──────►│        │  OAuth GitHub/Google│
│   ├─ adapter-sync : yrs, reqwest, ws    │        │        ├─ /sync : relais    │
│   └─ keyring : refresh token (OS)       │        │        │  y-sync + diffusion │
└─────────────────────────────────────────┘        │        └─ SQLite + litestream│
                                                   └────────────────────────────┘
```

Client : nouvelle crate `adapter-sync` dans [src-tauri/crates](../../src-tauri/crates) (yrs, reqwest,
keyring, websocket). Serveur : nouveau dossier `server/` dans ce repo (workspace Cargo séparé) ou
repo `daftr-server`. Mêmes conventions Rust ([clippy.toml](../../src-tauri/clippy.toml)).

Documents CRDT :

- `note:<uuid>` : `content` (`Y.Text`), `meta` (`Y.Map` : `tags`, `folder`, `sort_order`,
  `color`, `created_at`, `deleted_at`). Le titre reste dérivé de la première ligne.
- `workspace` : `folders` (`Y.Map` par `uuid` → `name`, `parent`, `sort_order`, `color`,
  `is_default`, `deleted_at`).

## 4. Phase 1 — modèle de données (sans serveur)

Objectif : l'app se comporte exactement comme aujourd'hui, mais chaque note et dossier a un
`uuid`, les suppressions laissent une trace, toute modification est versionnée pour la sync, et
chaque note possède son document `yrs` tenu à jour. Tout est testable sans réseau.

### 4.1 Dépendances à ajouter (crates `app` / `adapter-sqlite`)

| Crate | Pourquoi |
|---|---|
| `uuid` (`v4`, `serde`) | Identifiants stables entre appareils. |
| `yrs` | Document CRDT par note, même format que Yjs. |
| `similar` | Diff texte → opérations `Y.Text` tant que l'éditeur n'est pas lié à Yjs (phases 1 à 4). |

### 4.2 Migration SQLite — [schema.rs](../../src-tauri/crates/adapter-sqlite/src/schema.rs)

Nouvelle migration idempotente `add_sync_columns`, dans l'ordre :

1. `notes` : `uuid TEXT`, `deleted_at TEXT NULL`, `crdt BLOB NULL`,
   `sync_version INTEGER NOT NULL DEFAULT 0`, `synced_version INTEGER NOT NULL DEFAULT 0`.
2. `folders` : `uuid TEXT`, `deleted_at TEXT NULL`.
3. Remplissage des `uuid` manquants (boucle Rust, `uuid::Uuid::new_v4()`), puis index uniques
   `notes_uuid`, `folders_uuid`.
4. Triggers `notes_sync_au` / `folders_sync_au` : `sync_version = sync_version + 1` sur tout
   `UPDATE` (y compris `folder_id`, `sort_order`, `color`, qui ne touchent pas `updated_at` :
   voir [ports.rs:27](../../src-tauri/crates/app/src/ports.rs#L27)). Garde anti-récursion :
   `WHEN new.sync_version = old.sync_version`.
5. Table `sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL)` : `device_id`, `server_url`,
   `cursor`, `last_sync_at`. Le `device_id` est tiré à la première ouverture.
6. Migration des liens `note://<entier>` → `note://<uuid>` (§4.4), puis marqueur
   `sync_state.links_migrated = 1`.

Suppression : `NoteStore::delete` devient un soft delete (`deleted_at = now`, contenu conservé
jusqu'à la purge). Les lectures (`all`, `by_id`, `search`, `by_tag`, `tag_columns`) filtrent
`deleted_at IS NULL`. Le trigger FTS `notes_au` existant réindexe la note supprimée ; ajouter
une condition `WHEN new.deleted_at IS NULL` et un trigger qui retire de la FTS quand `deleted_at`
passe à non nul. La purge définitive (tombstones acquittés par le serveur depuis 30 jours)
arrive en phase 4 ; en attendant un dossier supprimé en cascade marque ses notes supprimées.

### 4.3 Domaine — [note.rs](../../src-tauri/crates/domain/src/note.rs), `folder.rs`

- `Note.uuid: String`, `Folder.uuid: String`. On **garde** `id: i64` comme poignée locale pour
  le front, les tabs, le MCP : rien ne change dans leurs contrats.
- `Note.deleted_at: Option<String>` n'est pas exposé au front (filtré avant).
- Exports `ts-rs` régénérés (le front reçoit `uuid` en plus).

### 4.4 Liens `note://`

Aujourd'hui l'identifiant est l'entier local ([note-links.util.ts:60](../../frontend/src/app/core/utils/note-links.util.ts#L60)).
Sur un second PC il désignerait une autre note. Les liens `[label](Dossier/Titre)` sont déjà
indépendants de l'appareil et ne bougent pas.

- Backend : fonction pure `domain::links::rewrite_note_links(content, &HashMap<i64, Uuid>)`,
  appliquée à toutes les notes en une transaction. Idempotente : un lien déjà en uuid est
  laissé tel quel.
- Front : la regex `NOTE_LINK` accepte `\d+|<uuid>`, les résolutions passent par une
  `Map<uuid, Note>`. Fichiers à adapter : `note-links.util.ts`, `link-navigation.ts`,
  `markdown-widgets.ts`, `palette.component.ts` (insertion de lien), `editor.component.ts`,
  `graph-view.component.ts`, `markdown-preview.component.ts`, `backlinks.service.ts`.
  Les tests `note-links.test.ts`, `link-navigation.test.ts`, `link-status.test.ts` à étendre.
- MCP : vérifier que les outils qui produisent des liens écrivent des uuid
  ([tools.rs](../../src-tauri/crates/adapter-mcp/src/tools.rs)).

### 4.5 Couche CRDT — module `app::sync`

Fonctions pures, sans I/O, donc testables à sec :

- `note_doc::from_note(&Note) -> Doc` : construit le document d'une note existante (migration).
- `note_doc::apply(doc, &NoteChanges)` : diff `similar` entre le texte du `Y.Text` et le nouveau
  contenu, converti en `insert`/`remove` positionnels ; `meta` mis à jour clé par clé.
- `note_doc::apply_placement(doc, folder_uuid, sort_order)`, `apply_color`, `apply_delete`.
- `note_doc::project(doc) -> NoteRow` : relit le document vers les colonnes SQLite (sert dès
  que des updates distantes arrivent, phase 4).
- `workspace_doc::*` : même chose pour les dossiers, plus `repair_cycles`.
- Identifiant client Yjs = dérivé de `device_id` (un seul processus par appareil, donc sûr).

`adapter-sqlite` tient le blob à jour dans la même transaction que les colonnes : lire `crdt`,
appliquer, réécrire. Le blob d'une note non encore migrée est construit à la volée par
`from_note`. Ce choix évite de changer les signatures de `NoteStore` ; seule la
responsabilité interne de l'adaptateur grossit.

### 4.6 Tests

- Migration sur une base de la v1.3.1 (fixture) : uuid uniques, liens réécrits, FTS cohérente,
  seconde exécution sans effet.
- Triggers : déplacer / colorer / repositionner bumpe `sync_version` sans toucher `updated_at`.
- Convergence : deux `Doc` partant du même état, éditions concurrentes sur la même ligne,
  échange d'updates dans les deux ordres → même contenu. Même test pour les dossiers avec un
  cycle provoqué.
- Diff → `Y.Text` : propriété `project(apply(doc, content)) == content` sur des contenus
  aléatoires (accents, emojis : attention aux indices UTF-16 vs UTF-8 dans `yrs`).
- Front : parsing et résolution des liens uuid, rétrocompatibilité des liens entiers.

Estimation : une semaine. Livrable : une release où rien ne change pour l'utilisateur.

## 5. Phases suivantes

| Phase | Contenu | Estimation |
|---|---|---|
| 2 — serveur | axum : `/auth/start`, `/auth/callback/{provider}`, `/auth/token` (PKCE, JWT 15 min + refresh opaque haché, rotation, révocation), `/sync` websocket (y-sync par document, diffusion aux autres appareils du même utilisateur), persistance SQLite (updates ajoutées puis compactées en snapshot), `/me/export`, `DELETE /me`. Quotas par utilisateur, limite de taille, rate limiting. | 1,5 à 2 semaines |
| 3 — compte dans l'app | Commandes `sign_in` / `sign_out` / `session`, listener loopback `127.0.0.1:<port>`, ouverture du navigateur via `tauri-plugin-opener`, refresh token dans `keyring`, écran Settings → Compte, URL du serveur modifiable. | 1 semaine |
| 4 — moteur de sync | File des `sync_version > synced_version`, pull par curseur au démarrage et à la reconnexion, websocket en continu, application des updates distantes puis `project` vers SQLite, purge des tombstones, indicateur d'état, premier login avec notes locales / premier lancement sur un second PC. | 1,5 semaine |
| 5 — temps réel dans l'éditeur | `y-codemirror.next` dans la webview, le `Doc` de la note ouverte passe par un pont Tauri vers le Rust ; curseurs des autres appareils. Optionnel. | 1 semaine |
| 6 — exploitation | Caddy + systemd + `litestream` vers OVH Object Storage, alertes basiques, page de confidentialité (GitHub Pages), README et notes de version. | 2 à 3 jours |

## 6. Serveur OVH : ce qu'il faut

- VPS Debian 12 (offre Starter/Value, 3,5 à 7 €/mois), un nom de domaine (~10 €/an), DNS vers le VPS.
- Caddy en reverse proxy : TLS automatique, HTTP/2, websockets.
- `daftr-server` en service systemd, utilisateur dédié, SQLite en WAL, `litestream` qui réplique
  en continu vers un bucket S3 OVH (quelques centimes par mois).
- Secrets (`GITHUB_CLIENT_SECRET`, `GOOGLE_CLIENT_SECRET`, `JWT_SECRET`) dans un fichier
  d'environnement lisible par le service uniquement, jamais dans le repo.
- Hébergement en France ou en Allemagne : données en UE pour le RGPD.

## 7. Reprise par quelqu'un d'autre

Possible, et prévu dès la conception :

- Les utilisateurs sont identifiés par `(provider, provider_subject)`. Le repreneur enregistre ses
  propres apps GitHub / Google, les utilisateurs se reconnectent, leur identité correspond, leurs
  données sont là.
- Migrer = arrêter le service, copier le fichier SQLite (ou restaurer la réplique litestream),
  relancer ailleurs. Le repreneur change l'URL par défaut dans l'app ou récupère le domaine.
- `/me/export` donne à chacun ses notes (obligation RGPD de portabilité de toute façon), et
  l'app garde tout en local : même sans migration, personne ne perd ses notes.

Engagement réel : une page de confidentialité, répondre aux demandes de suppression, et
prévenir avant de couper le serveur. Rien de plus.
