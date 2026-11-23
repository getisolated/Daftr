# Synchronisation entre appareils (réseau local)

> Référence de la feature livrée (branche `NF/feat/sync-lan-261123`, PR #90, 2026-11-23).
> Spec d'origine, gardée telle qu'écrite avant l'implémentation : [conception-lan.md](conception-lan.md).
> Mesures du spike (lot 0) : [mesures-spike.md](mesures-spike.md).
> Ce document remplace l'ancien plan (serveur OVH, connexion GitHub / Google), abandonné le 2026-11-19.

## 1. Vue d'ensemble

Daftr synchronise les notes d'un même utilisateur entre ses appareils, **en réseau local uniquement** : aucun serveur, aucun trafic Internet. Deux appareils échangent quand ils sont allumés en même temps sur le même réseau.

- **Confiance** : établie une fois par **appairage** (code à 6 chiffres, QR code, `IP:port` en repli). Elle est transitive au sein d'un profil partagé : un appareil ajouté par un membre se synchronise avec tous les autres.
- **Profils synchronisés** : choisis profil par profil et appareil par appareil. Tous les profils synchronisés tournent, actifs ou non, verrouillés compris (la base n'est pas chiffrée).
- **Automatique et en temps réel**, avec pause globale et « Synchroniser maintenant ». Synchronisation complète dès l'appairage ou l'acceptation d'un profil.
- **Pas de dialogue de conflit** : chaque note est un document CRDT (`yrs`, format Yjs), fusionné caractère par caractère. La note ouverte reçoit les modifications sans perdre la saisie en cours.
- **Coût nul sans profil synchronisé** : ni écoute, ni annonce, ni thread ; seule trace, `sync/device.key`.
- **Hors périmètre** : partage avec d'autres personnes, réglages, collections, mobile, historique des versions.

## 2. Architecture

| Crate | Rôle dans la synchronisation | Fichiers clés |
|---|---|---|
| `domain` | Types purs : `SpaceId`, `DeviceKey`, `DeviceName`, `PairingCode`, `DocId`, `Uuid` (v4/v5 maison), dossier par défaut dérivé, purge à 30 jours, réglages `syncDeviceName` / `syncPort` / `syncPaused`, `NoteChanges::base` | [`domain/src/sync.rs`](../../src-tauri/crates/domain/src/sync.rs), [`domain/src/note.rs`](../../src-tauri/crates/domain/src/note.rs) |
| `app::sync` | CRDT pur, sans I/O : documents de note et d'espace, projection déterministe, fusion d'un profil existant, diff texte → opérations, fusion à trois voies d'un texte | [`app/src/sync/`](../../src-tauri/crates/app/src/sync/) |
| `app::ports` | Ports `SyncStore`, `SyncProfiles`, `SyncEvents`, `TrustStore` ; doublures en mémoire (feature `testing`) | [`app/src/ports/sync.rs`](../../src-tauri/crates/app/src/ports/sync.rs), [`app/src/ports/trust.rs`](../../src-tauri/crates/app/src/ports/trust.rs) |
| `adapter-sqlite` | Migration, blobs CRDT tenus à jour par chaque écriture, `seq`, curseurs, `apply_remote` et projection en transaction, `adopt_space`, rebase des enregistrements | [`adapter-sqlite/src/sync/`](../../src-tauri/crates/adapter-sqlite/src/sync/) |
| `adapter-fs` | `sync/device.key`, `devices.json`, `index.json` (écriture atomique, lecture tolérante) | [`adapter-fs/src/sync_files.rs`](../../src-tauri/crates/adapter-fs/src/sync_files.rs) |
| `adapter-sync` | Moteur réseau, sans SQL : écoute filtrée, Noise, SPAKE2, mDNS, QR, appairage, sessions, rattrapage, temps réel, offres, backoff, pause | [`adapter-sync/src/`](../../src-tauri/crates/adapter-sync/src/) |
| `adapter-mcp` | Base de `update_note` : le contenu lu par l'IA | [`adapter-mcp/src/tools/reads.rs`](../../src-tauri/crates/adapter-mcp/src/tools/reads.rs) |
| `presentation` | DTO et bindings TypeScript (`ts-rs`) des commandes et événements | [`presentation/src/dto/sync.rs`](../../src-tauri/crates/presentation/src/dto/sync.rs) |
| `desktop` | Moteur unique par installation, `impl SyncProfiles`, commandes Tauri, événements, surveillance du réseau, hooks NSIS, `Info.plist` | [`desktop/src/sync.rs`](../../src-tauri/crates/desktop/src/sync.rs), [`desktop/src/commands/sync.rs`](../../src-tauri/crates/desktop/src/commands/sync.rs) |
| `frontend` | Réglages → Synchronisation, réglages du profil, indicateur, dialogue des offres, notifications, fusion dans l'éditeur | [`features/sync/`](../../frontend/src/app/features/sync/), [`sync.service.ts`](../../frontend/src/app/core/services/sync.service.ts), [`remote-changes.ts`](../../frontend/src/app/shared/codemirror/remote-changes.ts) |

Pile : `yrs` 0.28 (`OffsetKind::Utf16`), `similar` (diff), `snow` (`Noise_XX_25519_ChaChaPoly_BLAKE2s`), `spake2`, `mdns-sd` (Rust pur, sans Avahi ni Bonjour), `qrcode` (SVG), `postcard`, `if-addrs`.

## 3. Modèle de données

### Fichiers de l'appareil (`<données>/sync/`)

| Fichier | Contenu |
|---|---|
| `device.key` | Clé statique x25519 (32 octets bruts). L'identifiant de l'appareil est sa clé publique. `0600` sous Unix, ACL « utilisateur seul » (`icacls`) sous Windows. Créée au premier lancement, même sans synchronisation |
| `devices.json` | Appareils de confiance : clé, nom, origine (`paired` / `space`), ajout, `revoked_at` |
| `index.json` | Cache `space_id → profil`, reconstruit au démarrage depuis les bases |

Écriture atomique (fichier voisin puis renommage). Lecture tolérante : JSON illisible → liste vide et copie `.corrupted` ; clé illisible mise de côté, nouvelle identité ; une révocation n'est jamais perdue.

### Base de chaque profil

Migration idempotente `add_sync_columns`, précédée d'une sauvegarde `daftr.db.pre-sync` (`VACUUM INTO`) :
- `notes` : `uuid` (unique), `deleted_at` (suppression douce, filtrée partout, FTS comprise), `crdt` (blob), `seq`, `folder_uuid` ;
- `folders` : `uuid`, `deleted_at` ;
- `sync_docs` (document `space`), `sync_state` (`space_id`, `enabled`, `next_seq`, génération d'identifiant client), `sync_peers` (`acked_seq`, `last_addr`, `last_seen_at` par appareil distant).

Règles :
- `seq` local monotone, attribué à **chaque écriture de blob, locale ou distante** (propagation A→B→C). Tout passe par l'adaptateur, dans la transaction de l'écriture : le document rattrape la ligne écrite.
- Un profil non synchronisé n'a aucun blob : ses écritures sont celles de la v1.7.0.
- `id` reste la poignée locale ; `uuid` est l'identifiant commun. Les liens `note://<entier>` sont réécrits en `note://<uuid>` (backend, front, MCP).
- Le dossier par défaut prend l'uuid `UUIDv5(space_id, "default-folder")` : une seule Inbox après fusion.
- Purge : 30 jours après une suppression, le contenu est vidé, la tombstone reste. Une mise à jour tardive recrée la note.

### Documents CRDT

- `note:<uuid>` : `content` (`Y.Text`), `meta` (`tags`, `folder`, `sort_order`, `color`, `created_at`, `updated_at`, `deleted_at`, `deleted_sv`), `edits` (horloge de la dernière modification du contenu par client).
- `space` : `name`, `folders` (uuid → nom, parent, ordre, couleur, défaut, suppression), `devices` (clé → nom, ajout, révocation).
- Blob ré-encodé depuis l'état courant, GC de `yrs` actif. Identifiant client : clé de l'appareil pour créer ou rejoindre un espace, puis génération aléatoire par session d'écriture (une sauvegarde restaurée ne réutilise pas des horloges déjà émises).

### Fusion et projection (`app::sync`, sans I/O)

Deux appareils dans le même état CRDT produisent les mêmes lignes SQLite.
1. Texte : fusion `Y.Text`. Métadonnées : last-writer-wins, égalité d'ordre départagée par uuid.
2. Suppression contre modification : la note reste visible si une modification du contenu dépasse `deleted_sv`.
3. Dossier inconnu ou supprimé : note projetée à la racine, `folder_uuid` gardé, rangée à l'arrivée du dossier.
4. Cycle de dossiers : le plus petit uuid du cycle est rattaché à la racine (à la projection, sans écriture).
5. Fusion d'un profil existant (seulement s'il n'est pas déjà synchronisé) : sauvegarde `daftr.db.pre-merge-<date>`, dossiers de même chemin unifiés, Inbox unique, notes identiques dans le même chemin dédoublonnées, toutes les autres gardées.

### Enregistrement construit sur une base ancienne

L'éditeur envoie le contenu complet. Le backend le compare au contenu courant et n'applique au CRDT que le diff. Si une modification distante a été appliquée pendant qu'un enregistrement était en vol, le diff « courant → reçu » l'effacerait, puis la propagerait comme une suppression.

`notes_update` (et le port `NoteStore::update`) reçoivent donc la **base** : le texte sur lequel le contenu a été construit. Dans un profil synchronisé, dans la transaction d'écriture, si la base diffère du contenu courant, le contenu reçu est rejoué sur le courant par [`text_merge::merge`](../../src-tauri/crates/app/src/sync/text_merge.rs) (diffs base → courant et base → reçu, tissés position par position de la base) :
- un caractère de la base supprimé d'un côté est supprimé ;
- toutes les insertions sont gardées ; à la même position, la distante d'abord, puis la locale ; deux insertions identiques n'en font qu'une (renvoyer le texte courant ne change rien) ;
- un passage remplacé d'un côté et modifié de l'autre garde le remplacement et les insertions de l'autre, comme la fusion CRDT, sans marqueur de conflit.

Le titre est recalculé sur le texte fusionné, que `notes_update` renvoie. Sans base, ou dans un profil non synchronisé, le contenu reçu remplace le contenu courant (comportement d'avant).

Choix de la base : **le texte complet**, pas une empreinte. L'IPC est locale et la note y passe déjà entière ; le backend reste sans état ; le diff base → reçu a de toute façon besoin du texte. Une empreinte obligerait à garder les textes passés, et le document `yrs` (GC actif) ne permet pas de reconstruire l'état à une base sans désactiver le GC (blobs qui gonflent).

Bases utilisées :
- **éditeur** : le dernier texte chargé, enregistré ou fusionné. Les enregistrements partent un par un, dans l'ordre ; la réponse du dernier est fusionnée dans l'éditeur (base = texte envoyé). Une relecture de la note qu'un enregistrement a pu devancer est ignorée et refaite après la réponse : une liste lue avant un de nos enregistrements n'est jamais prise pour une modification ;
- **notes rapides** : le texte au début de l'édition ;
- **renommage des liens** : le contenu lu dans la liste ;
- **MCP** : `update_note` prend le dernier contenu que la session de l'IA a lu (`get_note`) ou écrit (32 notes retenues par session, par uuid) ; `append_to_note`, le contenu lu juste avant. Une note jamais lue est remplacée.

## 4. Protocole (`adapter-sync`)

### Transport et adresses

- Une écoute TCP par installation, port `syncPort` (47820 par défaut), toutes interfaces.
- **Toute IP source non privée est refusée avant le handshake** (RFC 1918, `169.254/16`, `fc00::/7`, `fe80::/10`, loopback pour les tests). Le moteur ne compose jamais une adresse non privée, même saisie.
- Noise `XX_25519_ChaChaPoly_BLAKE2s`, messages `postcard` découpés en trames Noise, 16 Mio maximum vérifiés avant allocation. Premier message : `Open(Pair | Sync)`. Version de protocole 1.
- Découverte mDNS `_daftr-sync._tcp.local.` : TXT avec empreinte de la clé publique et version ; `pairing=1` et nom pendant un appairage. **Jamais de code ni de `space_id`.** Détection périodique d'IP de `mdns-sd` coupée ; annonce refaite sur changement de réseau.

### Appairage

1. A : « Lier un appareil », choix des profils (actif coché). Un espace est créé pour chaque profil coché qui n'en a pas.
2. A affiche un code à 6 chiffres et un QR code `daftr-pair:1:<IP:port,…>:<empreinte>:<code>` (IPv6 entre crochets, 4 adresses au plus), valables 5 minutes. Nouveau code après 10 échecs. Le mode se ferme à la première liaison.
3. B : « Se lier à un appareil », saisie du code. B compose vers les appareils en mode appairage découverts (4 en parallèle au plus) ou vers l'`IP:port` saisi. Le QR est vérifié contre l'empreinte (scan prévu pour le mobile).
4. Vérification : Noise XX, puis SPAKE2 (code = mot de passe), puis confirmation par HMAC sur le hash du handshake : le code est lié à la connexion, un seul essai par tentative, rien à rejouer hors ligne.
5. A et B s'ajoutent dans `devices.json` (`paired`). A envoie l'offre (`space_id`, nom, nombre de notes par profil). B choisit par profil : nouveau profil, fusionner avec un profil non synchronisé, ignorer. A ajoute B dans `space.devices`, envoie un `Snapshot` (B l'adopte par `adopt_space`), et la session passe en `Sync` sur la même connexion.

Offre ultérieure (« Synchroniser avec… » dans les réglages du profil) : envoyée tout de suite si l'appareil est connecté, sinon dès son apparition.

### Session `Sync`

- **Une connexion par paire d'appareils**, pour tous leurs profils communs. Clé distante connue et non révoquée exigée. En cas de collision, l'appareil à la plus petite clé initie.
- `Hello` : chaque côté n'annonce que les espaces où l'autre est membre non révoqué ; on synchronise l'intersection, moins les profils désactivés.
- **Rattrapage** par espace, dans chaque sens : `Changed` (documents avec `seq > acked_seq`, `space` en premier) → `Diffs` → `Updates` → `Done {seq}` → `Ack {seq}`. Application par lots de 200 documents (4 Mio) par transaction, événements regroupés. Premier contact : tout est échangé.
- **Temps réel** : chaque écriture locale (éditeur, MCP, overlay) appelle `local_change` ; après 1 s de regroupement, un rattrapage ciblé part vers les appareils connectés. Après `apply_remote`, la session relaie aux autres sessions (A→B→C), sauf à celle d'origine.
- **Déclencheurs**, sans polling réseau : fin d'appairage ou offre acceptée, apparition mDNS d'un appareil de confiance, changement de réseau (adresses locales relues toutes les 30 s quand la synchronisation tourne), écriture locale, « Synchroniser maintenant ». Échec : backoff de 30 s à 10 min vers `last_addr`.
- **Pause** : connexions fermées, écoute et annonce arrêtées ; les modifications partent à la reprise.

### Révocations

- « Arrêter avec X » (réglages du profil) : `revoked_at` dans `space.devices`. X ne reçoit plus ce profil.
- « Retirer » un appareil de confiance : révocation dans `devices.json` et dans tous les espaces communs. Définitif : le relier demande un nouvel appairage.
- Dans les deux cas, **les notes déjà copiées restent sur l'autre appareil**, et l'interface le dit.

## 5. Intégration

### Commandes Tauri

| Commande | Paramètres | Rend |
|---|---|---|
| `sync_state` | — | `SyncStateDto` (statut, clé et nom de l'appareil, port, port d'écoute, pause, profils synchronisés) |
| `sync_devices` | — | `TrustedDeviceDto[]` (présent, connecté, profils partagés avec `lastSyncAt`) |
| `sync_profile` | `profileId` | `ProfileSyncDto` (espace, activation, membres) |
| `sync_start_pairing` / `sync_stop_pairing` | `profileIds[]` / — | `PairingTicketDto` (code, uri, `qrSvg`, `expiresAt`) / — |
| `sync_pair` | `code`, `address?` (`IP:port`, `[IPv6]:port`, IP seule) | `LinkedDeviceDto` |
| `sync_answer_offer` | `deviceKey`, `spaceId`, `choice` (`new` / `merge` / `ignore`), `profileId?` | — |
| `sync_offer_profile`, `sync_stop_sharing` | `profileId`, `deviceKey` | — |
| `sync_remove_device` | `deviceKey` | — |
| `sync_set_profile_enabled` | `profileId`, `enabled` | — |
| `sync_now` | — | — |
| `sync_set_paused`, `sync_set_device_name`, `sync_set_port` | valeur | `SyncStateDto` |
| `notes_restore` | `id` | la note rétablie (note supprimée sur un autre appareil) |

Erreurs : `IpcError` de code `sync`, message = clé stable (`wrong_code`, `not_pairing`, `no_pairing_device`, `unreachable`, `timeout`, `not_local`, `listen_failed`, `discovery_failed`, `untrusted`, `unknown_offer`, `paused`, `fingerprint_mismatch`, `invalid_qr`, `protocol_version`, `protocol_failed`, `stopped`), traduite par le front.

Événements : `sync:status`, `sync:notes-synced` (avec `notes:changed` pour le profil actif), `sync:progress`, `sync:initial-done`, `sync:device-linked`, `sync:offer-received`, `sync:pairing`.

Réglages de l'appareil (`settings.json`, appliqués à chaud) : `syncDeviceName` (vide = nom d'hôte), `syncPort` (47820, de 1024 à 65535), `syncPaused`. Le front passe par `settings_set`.

### Interface

- **Réglages → Synchronisation** : état et « Synchroniser maintenant », nom de l'appareil, port, pause ; « Lier un appareil » (profils, QR, code, compte à rebours, adresses pour la saisie manuelle) ; « Se lier à un appareil » (code, `IP:port`) ; appareils de confiance (présent / connecté / absent, profils, dernière synchro, « Retirer »).
- **Dialogue des profils proposés** : destination par profil, puis « Synchronisation initiale : 120 / 450 notes ».
- **Réglages du profil** : activer, « Synchronisé avec : … », « Synchroniser avec… », « Arrêter avec X ». Appliqués tout de suite.
- **Indicateur** dans le pied de la barre latérale (en pause, à jour, synchronisation…, aucun appareil sur ce réseau, erreur avec la cause en tooltip). Caché sans profil synchronisé.
- **Notifications** in-app : appareil lié, offre reçue (« Choisir »), synchronisation initiale terminée.
- **Éditeur** : la note ouverte reçoit les modifications par une transaction minimale (curseurs et sélections mappés, hors historique d'annulation), rebasées sur la saisie non enregistrée ; badge « Modifié depuis <appareil> » 8 s. Même chemin pour le MCP. Note ouverte supprimée ailleurs : lecture seule, bandeau et « Restaurer » (la saisie non enregistrée est gardée).

## 6. Sécurité

- Transport chiffré et authentifié des deux côtés (Noise XX, clés statiques) ; seules les adresses privées sont acceptées ou composées.
- Appairage par PAKE lié au handshake ; code renouvelé après 10 échecs ; aucun secret ni `space_id` dans l'annonce mDNS.
- Les `space_id` ne sont révélés qu'aux appareils authentifiés et membres de l'espace.
- Entrées réseau non fiables : tailles bornées (message 16 Mio, offre 64 profils, 1 024 espaces), uuid et références validés, pas d'`unwrap` sur une entrée réseau.
- Données au repos inchangées : SQLite en clair ; le mot de passe d'un profil verrouille l'accès dans l'app, pas les fichiers.
- SPAKE2 en variante magic-wormhole (crate `spake2`), pas RFC 9382 : la construction est documentée dans [`pake.rs`](../../src-tauri/crates/adapter-sync/src/pake.rs) pour un futur client mobile.

## 7. Plateformes

- **Windows** : les hooks NSIS ([`hooks.nsh`](../../src-tauri/crates/desktop/windows/hooks.nsh)) ajoutent deux règles entrantes, TCP 47820 et UDP 5353, profil **Privé**, limitées à `Daftr.exe`, par `netsh` élevé : une invite UAC, une seule fois (marqueur `HKCU\Software\Daftr\Sync`). Elles sont retirées à la désinstallation (pas lors d'une mise à jour). Un réseau « Public » bloque : l'indicateur affiche `listen_failed`.
- **macOS** : [`Info.plist`](../../src-tauri/crates/desktop/Info.plist) déclare `NSLocalNetworkUsageDescription` et `NSBonjourServices` (`_daftr-sync._tcp`) ; macOS demande l'autorisation « réseau local » au premier usage.
- **Linux** : rien d'automatique ; avec ufw ou firewalld, ouvrir le TCP 47820 et l'UDP 5353 (voir le README).

## 8. Budgets mesurés

Mesures du spike, dans l'app réelle pour le binaire ([mesures-spike.md](mesures-spike.md)) :

| Budget | Objectif | Mesuré |
|---|---|---|
| Binaire | < 1 Mo | **+1,69 Mio** (`mdns-sd` 46 %, `yrs` 29 %) : dépassement accepté le 2026-11-19 |
| RAM au repos (écoute + mDNS) | < 5 Mo | +0,7 à +0,9 Mio d'octets privés |
| CPU au repos | ≈ 0 | 0 ms sur 5 min avec la détection d'IP de `mdns-sd` coupée (fait) |
| Sans profil synchronisé | v1.7.0 | rien ne tourne (testé : ni écoute, ni annonce, ni thread) |

Pistes pour repasser sous 1 Mo, non implémentées : mDNS minimal maison, diff maison à la place de `similar`, Noise SHA256 via `ring` (déjà embarqué).

## 9. Écarts assumés par rapport à la spec

Modèle et CRDT :
- Documents de note avec `updated_at` et une racine `edits` (projection déterministe, règle suppression contre modification).
- `adopt_space` reçoit aussi les notes distantes ; cycles réparés à la projection, sans écriture.
- Identifiant client Yjs : 53 bits de la clé pour créer ou rejoindre un espace, puis génération aléatoire par session d'écriture.
- Enregistrement avec base et fusion à trois voies côté backend (§3), à la place de « vider la sauvegarde en attente » (§8 de la spec).

Protocole :
- Le temps réel relance un rattrapage ciblé (`Changed` / `Diffs` / `Updates` / `Done` / `Ack`) au lieu d'un `Update` direct.
- Offre acceptée : `Snapshot` puis `adopt_space`. Offres en attente gardées en mémoire, perdues au redémarrage (moteur et front).
- Mode appairage fermé à la première liaison ; `last_addr` enregistrée pour les connexions sortantes seulement ; pas de keepalive.
- Un appareil sans profil synchronisé n'écoute pas : il ne reçoit une offre que s'il compose (« Se lier à un appareil »).

Intégration et interface :
- Événements préfixés `sync:` ; un profil créé par une offre n'émet pas d'événement de liste : le front relit `profiles_state`.
- Nom proposé d'un profil sans nom : « Mes notes » / « My notes ».
- `device.key` créé au premier lancement, même sans synchronisation.
- Changement de réseau détecté en relisant les adresses toutes les 30 s (les API natives demandent du `unsafe` ou une dépendance lourde).
- Indicateur dans la barre latérale (pas de barre d'état), caché sans profil synchronisé.
- Port et pause par les contrôles génériques des réglages (`settings_set`).
- Progression en documents (notes + document `space`) affichée comme « notes ».
- Pas de notification système : notifications dans l'interface.

## 10. Limites connues et points d'attention

- 🟠 **Pare-feu Windows** : la règle demande une élévation UAC à l'installation. Refusée, Windows pose sa propre question à la première écoute. Un port changé dans les réglages n'est pas couvert par la règle.
- 🟠 **Profils inactifs** rouverts à chaque accès du moteur (migration vérifiée, purge) : coût à mesurer sur de gros profils pendant une synchronisation initiale.
- 🟠 Changer le port pendant un appairage laisse l'ancien port dans le QR code (le code reste valable via mDNS).
- Chevauchement d'éditions concurrentes sur le même passage : toutes les insertions gardées, sans marqueur (règles du §3). Une IA qui écrit une note sans l'avoir lue dans sa session la remplace.
- Profil non synchronisé : pas de fusion côté backend (comportement v1.7.0) ; l'éditeur fusionne toujours ce que le MCP écrit, sauf si l'écriture tombe pendant un enregistrement en vol.
- Une modification distante arrivée pendant un enregistrement s'affiche à la réponse de celui-ci (au plus le temps d'un enregistrement).
- Backoff limité à `last_addr` ; interfaces virtuelles (Hyper-V, WSL, VPN) écartées par heuristique de noms.
- La confirmation « Arrêter avec X » s'ouvre au-dessus du dialogue du profil : Échap ferme les deux.
- `NSLocalNetworkUsageDescription` en anglais seulement.

Non vérifié (à faire en recette sur deux PC réels) : critères d'acceptation 1 à 7 de la spec, invite et règles du pare-feu Windows (le hook est seulement compilé), autorisation « réseau local » de macOS, rendu des écrans, ordre réel des événements `sync:notes-synced` / `notes:changed`, historique d'annulation dans l'app, taille du binaire final.

## 11. Tests

- **Données** (`adapter-sqlite`) : migration sur fixture, soft delete et FTS, blobs tenus à jour, échanges entre bases (mêmes lignes), fusion d'un profil existant, base restaurée, enregistrement en vol rejoué sur la modification distante, profil non synchronisé inchangé.
- **CRDT** (`app`) : convergence (propriété, N appareils, ordre aléatoire, doublons), contenu exact (UTF-16, emojis, CRLF), suppression contre modification, dossiers et cycles, fusion à trois voies (propriété : éditions disjointes toutes gardées ; base = courant → texte reçu ; idempotence).
- **Protocole** (`adapter-sync`) : appairage (bon et mauvais code, appairages simultanés, 10 échecs, expiration, QR d'une autre empreinte), sessions multi-espaces A/B/C, propagation transitive, offres différées, refus d'IP non privée et de clé révoquée, message trop gros, pause et reprise.
- **Intégration** (`desktop`) : fichiers `sync/`, `SyncProfiles`, démarrage sans rien qui tourne, et bout en bout à trois installations sur loopback.
- **MCP** : `update_note` garde ce qui a changé depuis la lecture de l'IA.
- **Front** (vitest) : diff minimal, fusion à trois voies dans l'éditeur, curseur mappé, historique, statuts, erreurs traduites, compte à rebours, destinations, notifications.

## 12. Phases ultérieures

Client mobile (scan du QR code, même format), scan du QR par webcam sur desktop, export / import d'un paquet de modifications par fichier pour les appareils jamais présents ensemble, historique des versions, `y-codemirror.next` (Yjs dans l'éditeur, ce qui rendrait la fusion à trois voies inutile), offres persistées, mDNS allégé pour revenir sous 1 Mo, keepalive.
