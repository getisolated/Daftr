# Synchronisation entre appareils (réseau local)

> Référence de la feature livrée (branche `NF/feat/sync-lan-261123`, PR #90, 2026-11-23).
> Spec d'origine, gardée telle qu'écrite avant l'implémentation : [conception-lan.md](conception-lan.md).
> Mesures du spike (lot 0) : [mesures-spike.md](mesures-spike.md).
> Ce document remplace l'ancien plan (serveur OVH, connexion GitHub / Google), abandonné le 2026-11-19.

## 1. Vue d'ensemble

Daftr synchronise les notes d'un même utilisateur entre ses appareils, **en réseau local uniquement** : aucun serveur, aucun trafic Internet. Deux appareils échangent quand ils sont allumés en même temps sur le même réseau.

- **Confiance** : établie une fois par **appairage** (code à 6 chiffres, QR code, `IP:port` en repli). Elle est transitive au sein d'un profil partagé : un appareil ajouté par un membre se synchronise avec tous les autres.
- **Profils synchronisés** : choisis profil par profil et appareil par appareil, sans interrupteur : un profil est synchronisé dès qu'il est partagé avec un appareil. Tous les profils synchronisés tournent, actifs ou non, verrouillés compris (la base n'est pas chiffrée).
- **Automatique et en temps réel**, avec pause globale et « Synchroniser maintenant ». Synchronisation complète dès l'appairage ou l'acceptation d'un profil.
- **Pas de dialogue de conflit** : chaque note est un document CRDT (`yrs`, format Yjs), fusionné caractère par caractère. La note ouverte reçoit les modifications sans perdre la saisie en cours.
- **Coût nul sans profil synchronisé** : ni écoute, ni annonce, ni thread ; seule trace, `sync/device.key`.
- **Hors périmètre** : partage avec d'autres personnes, réglages, mobile, historique des versions. Les collections (dossiers de fichiers `.md`) ont leur propre plan, [collections.md](collections.md), et leur section ici (§12).

## 2. Architecture

| Crate | Rôle dans la synchronisation | Fichiers clés |
|---|---|---|
| `platform` | Présence d'une batterie système (`power`), pour le type de l'appareil | [`platform/src/power.rs`](../../src-tauri/crates/platform/src/power.rs) |
| `domain` | Types purs : `SpaceId`, `DeviceKey`, `DeviceName`, `DeviceKind`, `PairingCode`, `DocId`, `Uuid` (v4/v5 maison), dossier par défaut dérivé, purge à 30 jours, réglages `syncDeviceName` / `syncPort` / `syncPaused`, `NoteChanges::base` | [`domain/src/sync.rs`](../../src-tauri/crates/domain/src/sync.rs), [`domain/src/note.rs`](../../src-tauri/crates/domain/src/note.rs) |
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
| `devices.json` | Appareils de confiance : clé, nom, type (`laptop` / `desktop` / `unknown`, absent des fichiers plus anciens : `unknown`), origine (`paired` / `space`), ajout, `revoked_at` |
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
- Noise `XX_25519_ChaChaPoly_BLAKE2s`, messages `postcard` découpés en trames Noise, 16 Mio maximum vérifiés avant allocation. Premier message : `Open(Pair | Sync)`. Version de protocole 1 (inchangée par l'ajout du type d'appareil, voir plus bas).
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
- `Hello` : version, nom, espaces et **type d'appareil**. Chaque côté n'annonce que les espaces où l'autre est membre non révoqué ; on synchronise l'intersection, moins les profils désactivés (drapeau `enabled` gardé côté moteur, sans interrupteur dans l'interface).
- **Rattrapage** par espace, dans chaque sens : `Changed` (documents avec `seq > acked_seq`, `space` en premier) → `Diffs` → `Updates` → `Done {seq}` → `Ack {seq}`. Application par lots de 200 documents (4 Mio) par transaction, événements regroupés. Premier contact : tout est échangé.
- **Temps réel** : chaque écriture locale (éditeur, MCP, overlay) appelle `local_change` ; après 1 s de regroupement, un rattrapage ciblé part vers les appareils connectés. Après `apply_remote`, la session relaie aux autres sessions (A→B→C), sauf à celle d'origine.
- **Déclencheurs**, sans polling réseau : fin d'appairage ou offre acceptée, apparition mDNS d'un appareil de confiance, changement de réseau (adresses locales relues toutes les 30 s quand la synchronisation tourne), écriture locale, « Synchroniser maintenant ». Échec : backoff de 30 s à 10 min vers `last_addr`.
- **Pause** : connexions fermées, écoute et annonce arrêtées ; les modifications partent à la reprise.

### Type d'appareil

- `Hello` porte en dernier champ le type de l'émetteur, un octet : 0 inconnu, 1 portable, 2 fixe (téléphone, tablette : codes suivants).
- **Pas de changement de `PROTOCOL_VERSION`** : une version antérieure ignore l'octet en trop (postcard ne vérifie pas la fin du message), et un `Hello` sans cet octet, ou avec un code inconnu, se lit « inconnu ». Les deux versions continuent de se synchroniser ; la changer aurait coupé la synchronisation entre versions pour une simple icône.
- Détection locale au démarrage (`platform::power`), d'après la présence d'une batterie système : portable avec, fixe sans, inconnu si la détection échoue.
  - Windows : `GetSystemPowerStatus` (`BatteryFlag` 128 : pas de batterie ; 255 : inconnu). Seul `unsafe` du crate, documenté.
  - Linux : une entrée `BAT*` ou de type `Battery` dans `/sys/class/power_supply`, hors périphériques (`scope` = `Device`) ; dossier illisible : inconnu.
  - macOS : `InternalBattery` dans `/usr/bin/pmset -g batt` (`hw.model` ne distingue pas les Mac Apple Silicon portables).
- Le type reçu est gardé avec l'appareil de confiance (`devices.json`) à chaque `Hello`, par une lecture-écriture indivisible (`TrustStore::set_device_kind`) : une révocation écrite entre-temps n'est jamais perdue. Un appareil connu par un profil commun reste « inconnu » jusqu'à son premier `Hello`.

### Révocations

- « Arrêter avec X » (réglages du profil) : `revoked_at` dans `space.devices`. X ne reçoit plus ce profil.
- « Retirer » un appareil de confiance : révocation dans `devices.json` et dans tous les espaces communs. Définitif : le relier demande un nouvel appairage.
- Dans les deux cas, **les notes déjà copiées restent sur l'autre appareil**, et l'interface le dit.

## 5. Intégration

### Commandes Tauri

| Commande | Paramètres | Rend |
|---|---|---|
| `sync_state` | — | `SyncStateDto` (statut, clé et nom de l'appareil, port, port d'écoute, pause, profils synchronisés) |
| `sync_devices` | — | `TrustedDeviceDto[]` (type, empreinte, présent, connecté, profils partagés avec `lastSyncAt`) |
| `sync_profile` | `profileId` | `ProfileSyncDto` (espace, membres avec type et empreinte) |
| `sync_start_pairing` / `sync_stop_pairing` | `profileIds[]` / — | `PairingTicketDto` (code, uri, `qrSvg`, `expiresAt`) / — |
| `sync_pair` | `code`, `address?` (`IP:port`, `[IPv6]:port`, IP seule) | `LinkedDeviceDto` |
| `sync_answer_offer` | `deviceKey`, `spaceId`, `choice` (`new` / `merge` / `ignore`), `profileId?` | — |
| `sync_offer_profile`, `sync_stop_sharing` | `profileId`, `deviceKey` | — |
| `sync_remove_device` | `deviceKey` | — |
| `sync_now` | — | — |
| `sync_set_paused`, `sync_set_device_name`, `sync_set_port` | valeur | `SyncStateDto` |
| `notes_restore` | `id` | la note rétablie (note supprimée sur un autre appareil) |

Erreurs : `IpcError` de code `sync`, message = clé stable (`wrong_code`, `not_pairing`, `no_pairing_device`, `unreachable`, `timeout`, `not_local`, `listen_failed`, `discovery_failed`, `untrusted`, `unknown_offer`, `paused`, `fingerprint_mismatch`, `invalid_qr`, `protocol_version`, `protocol_failed`, `stopped`), traduite par le front.

Événements : `sync:status`, `sync:notes-synced` (avec `notes:changed` pour le profil actif), `sync:progress`, `sync:initial-done`, `sync:device-linked`, `sync:offer-received`, `sync:pairing`.

Réglages de l'appareil (`settings.json`, appliqués à chaud) : `syncDeviceName` (vide = nom d'hôte), `syncPort` (47820, de 1024 à 65535), `syncPaused`. Le front passe par `settings_set`.

### Interface

- **Réglages → Synchronisation** : état et « Synchroniser maintenant », nom de l'appareil, port, pause ; « Lier un appareil » (profils, QR, code, compte à rebours, adresses pour la saisie manuelle) ; « Se lier à un appareil » (code, `IP:port`) ; appareils de confiance (présent / connecté / absent, profils, dernière synchro, « Retirer »).
- **Dialogue des profils proposés** : destination par profil, puis « Synchronisation initiale : 120 / 450 notes ».
- **Réglages du profil** : une ligne « Appareils et synchronisations » (même présentation que « Définir un mot de passe maître », cachée à la création d'un profil) remplace le contenu du dialogue par une sous-vue, avec un retour (bouton ou Échap) qui garde les champs saisis. Pas d'Enregistrer / Supprimer / Annuler dans la sous-vue : tout s'applique tout de suite.
  - En haut, un QR code centré et grand, le code à 6 chiffres dessous, puis le compte à rebours en petit. Ouvrir la sous-vue ouvre le mode appairage pour ce profil (`startPairing([profil])`, qui lui crée un espace au besoin) ; après une liaison ou à l'expiration, un nouveau code est demandé tout seul. Quitter la sous-vue (retour, Échap, fermeture du dialogue) ferme le mode. Chargement et erreur (avec « Réessayer ») à la place du code.
  - Un seul écran ouvre le mode à la fois : le service retient qui l'a ouvert (`settings` / `profile`), chaque écran n'affiche et ne ferme que le sien, et seule la sous-vue renouvelle le code.
  - « Saisir le code d'un autre appareil » remplace le code par la saisie (code, adresse facultative), le composant partagé avec Réglages → Synchronisation ; le mode appairage de cet appareil est fermé pendant la saisie.
  - Dessous, les appareils avec lesquels le profil est synchronisé : icône du type (portable, écran, appareil générique), nom, et dessous un identifiant court, les 8 premiers chiffres de l'empreinte de la clé (`4F2A-9C1B`) ; « Arrêter » (confirmation) ne retire l'appareil que de ce profil. « Synchroniser avec… » propose le profil à un appareil déjà lié.
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
- Pas d'activation par profil : un profil est synchronisé dès qu'il est partagé (commande `sync_set_profile_enabled` retirée). Le drapeau `enabled` reste dans la base et le moteur ; l'appairage et les offres le remettent à vrai.
- L'identifiant affiché sous le nom d'un appareil est le début de l'empreinte de sa clé, pas l'adresse MAC : le protocole ne la connaît pas, et elle change selon le réseau Wi-Fi (adresse aléatoire) et la carte réseau.

## 10. Limites connues et points d'attention

- 🟠 **Pare-feu Windows** : la règle demande une élévation UAC à l'installation. Refusée, Windows pose sa propre question à la première écoute. Un port changé dans les réglages n'est pas couvert par la règle.
- 🟠 **Profils inactifs** rouverts à chaque accès du moteur (migration vérifiée, purge) : coût à mesurer sur de gros profils pendant une synchronisation initiale.
- 🟠 Changer le port pendant un appairage laisse l'ancien port dans le QR code (le code reste valable via mDNS).
- Chevauchement d'éditions concurrentes sur le même passage : toutes les insertions gardées, sans marqueur (règles du §3). Une IA qui écrit une note sans l'avoir lue dans sa session la remplace.
- Profil non synchronisé : pas de fusion côté backend (comportement v1.7.0) ; l'éditeur fusionne toujours ce que le MCP écrit, sauf si l'écriture tombe pendant un enregistrement en vol.
- Une modification distante arrivée pendant un enregistrement s'affiche à la réponse de celui-ci (au plus le temps d'un enregistrement).
- Backoff limité à `last_addr` ; interfaces virtuelles (Hyper-V, WSL, VPN) écartées par heuristique de noms.
- Ouvrir « Appareils et synchronisations » crée l'espace du profil (comme « Lier un appareil ») : le moteur démarre et l'indicateur apparaît même si aucun appareil ne rejoint.
- Un profil désactivé avec l'ancienne case à cocher le reste jusqu'à l'ouverture de sa sous-vue ou à une offre (plus d'interrupteur pour le réactiver).
- Le type d'un appareil n'est détecté qu'au démarrage, et seulement d'après la batterie : un fixe avec un onduleur vu comme batterie système passerait pour un portable.
- `NSLocalNetworkUsageDescription` en anglais seulement.

Non vérifié (à faire en recette sur deux PC réels) : critères d'acceptation 1 à 7 de la spec, invite et règles du pare-feu Windows (le hook est seulement compilé), autorisation « réseau local » de macOS, rendu des écrans (dont la sous-vue « Appareils et synchronisations » : QR code, renouvellement du code, retour, Échap), détection du type sous Linux et macOS, ordre réel des événements `sync:notes-synced` / `notes:changed`, historique d'annulation dans l'app, taille du binaire final.

## 11. Tests

- **Données** (`adapter-sqlite`) : migration sur fixture, soft delete et FTS, blobs tenus à jour, échanges entre bases (mêmes lignes), fusion d'un profil existant, base restaurée, enregistrement en vol rejoué sur la modification distante, profil non synchronisé inchangé.
- **CRDT** (`app`) : convergence (propriété, N appareils, ordre aléatoire, doublons), contenu exact (UTF-16, emojis, CRLF), suppression contre modification, dossiers et cycles, fusion à trois voies (propriété : éditions disjointes toutes gardées ; base = courant → texte reçu ; idempotence).
- **Protocole** (`adapter-sync`) : `Hello` avec type (aller-retour, `Hello` sans type ou de code inconnu lu « inconnu », lu par une version antérieure), type gardé par les deux appareils après l'appairage, appairage (bon et mauvais code, appairages simultanés, 10 échecs, expiration, QR d'une autre empreinte), sessions multi-espaces A/B/C, propagation transitive, offres différées, refus d'IP non privée et de clé révoquée, message trop gros, pause et reprise.
- **Fichiers** (`adapter-fs`) : `devices.json` sans type (ancien) ou de type inconnu, type enregistré et mis à jour.
- **Plateforme** (`platform`) : lecture de `BatteryFlag`, de `sysfs` (dossier temporaire) et de `pmset` ; repli « inconnu ».
- **Intégration** (`desktop`) : fichiers `sync/`, `SyncProfiles`, démarrage sans rien qui tourne, et bout en bout à trois installations sur loopback.
- **MCP** : `update_note` garde ce qui a changé depuis la lecture de l'IA.
- **Front** (vitest) : diff minimal, fusion à trois voies dans l'éditeur, curseur mappé, historique, statuts, erreurs traduites, compte à rebours, destinations, notifications, identifiant court, icône par type, renouvellement du code.

## 12. Collections partagées

Backend livré (lots 0 à 3 de [collections.md](collections.md), qui garde les décisions et le parcours utilisateur). Les collections sont des dossiers de fichiers `.md` du disque : on ne synchronise pas du contenu en base mais des **fichiers**, par-dessus les canaux existants (session chiffrée, appareils de confiance, espace du profil). Le front (lots 4 et 5) n'est pas livré.

### 12.1 Modèle de données

| Élément | Contenu |
|---|---|
| Réglage `receivedCollectionsFolder` (`settings.json`, `SettingsDto`) | Dossier des collections reçues, **par appareil, jamais synchronisé**. Vide : `Documents/Daftr` (dossier Documents résolu par Tauri, repli : dossier des données). Sinon un chemin absolu (refusé sinon, valeur précédente gardée). |
| `collection_shares` (base du profil) | `sync_id` (uuid, identifiant commun aux appareils) ↔ `path` de la collection (clé étrangère `ON UPDATE CASCADE ON DELETE CASCADE` : le partage suit un dossier déplacé et disparaît avec sa collection). |
| `collection_share_peers` | Appareil, état `offered` / `active`, dernier échange complet. |
| `collection_files` | Index des fichiers locaux : chemin, empreinte, taille, date. Évite de relire un fichier qui n'a pas bougé (un fichier modifié depuis moins de 2 s est toujours relu : certains systèmes datent à la seconde). |
| `collection_bases` | **Dernière version échangée avec chaque appareil**, par fichier : empreinte (SHA-256) et texte (UTF-8, 256 Kio au plus) pour la fusion à trois voies. |

Migration : quatre `CREATE TABLE IF NOT EXISTS` à l'ouverture de la base (idempotent, sans sauvegarde : aucune table existante n'est touchée). Tests : base 1.7.0 migrée deux fois, partage / appareils / bases / index, cascades.

### 12.2 Décision par fichier (`app::collection_sync`, pur)

Trois empreintes par chemin : locale `L`, distante `R`, base `B`. La base effective est celle que les deux côtés annoncent à l'identique ; si un accusé s'est perdu, celle qu'un côté annonce et que l'autre a exactement dans son fichier ; sinon aucune.

| `L` / `R` / `B` | Décision |
|---|---|
| `L = R` | rien (la base est retenue ; absent des deux côtés : oubliée) |
| un seul côté a le fichier, sans base | l'autre le reçoit |
| les deux ont le fichier, sans base | conflit |
| `L = B`, `R` absent | suppression (corbeille) |
| `L = B`, `R ≠ B` | je reçois |
| `L ≠ B`, `R = B` | l'autre reçoit (ou supprime) |
| `L` absent, `R ≠ B` / `L ≠ B`, `R` absent | **la modification gagne sur la suppression** |
| `L ≠ B`, `R ≠ B`, `L ≠ R` | conflit |

La décision est symétrique : chaque côté la calcule seul à partir des deux manifestes, ce que l'un reçoit l'autre l'envoie.

**Conflit.** Le plus petit identifiant d'appareil est l'**arbitre**, ce qui donne la même issue aux deux côtés sans échange :
- base connue et trois textes UTF-8 : fusion à trois voies par `text_merge::merge`, avec l'ordre « arbitre d'abord » (le résultat est identique des deux côtés), écrit par les deux ;
- sinon (premier échange, binaire, base trop grosse) : l'arbitre garde sa version à la place d'origine, le **suiveur** met la sienne dans `nom (conflit <son appareil>).md` (numéroté si le nom est pris) puis prend celle de l'arbitre. Rien n'est perdu ; la copie se synchronise ensuite comme un fichier nouveau.

**Chemins** (`SyncPath`) : relatifs, séparateurs `/`, refus de `..`, `.`, segments vides, chemin absolu, `\`, lecteur et flux (`:`), caractères de contrôle et `* ? " < > |`, noms réservés Windows (`CON`, `PRN`, `AUX`, `NUL`, `COM0-9`, `LPT0-9`, avec ou sans extension), nom finissant par un point ou une espace, plus de 255 octets par nom ou 1 024 au total, dossiers exclus ou cachés, profondeur au-delà de `MAX_SCAN_DEPTH`, fichier non `.md`. Deux fichiers qui ne diffèrent que par la casse sont **écartés tous les deux** et signalés (`caseCollision`) ; un fichier de plus de 10 Mio l'est aussi (`tooLarge`). Les fichiers qui ne sont pas des `.md` ne sont jamais copiés, modifiés ni supprimés.

Tests : tables de décision, chemins, noms de copies, fusion identique des deux côtés, et `proptest` sur deux copies simulées (accusés perdus, ordres d'application différents) : convergence toujours, aucun contenu non encore échangé perdu (fichiers binaires, jamais fusionnés).

### 12.3 Protocole

**Compatibilité.** `PROTOCOL_VERSION` reste à 1. postcard encode l'indice de variante en varint et **décode un indice inconnu comme une erreur** : une version antérieure fermerait la session à la réception d'un message de collection. Deux protections :
- `Hello` porte, après le type d'appareil, un octet `features` (bit 0 : collections). Une version antérieure ignore les octets en trop (comme pour le type) ; un `Hello` sans cet octet se lit « aucune fonctionnalité ». **Les messages de collections ne partent qu'à un appareil qui annonce le bit** : entre une ancienne et une nouvelle version, la synchronisation des notes continue à l'identique.
- `Message::decode` rend `Error::UnknownMessage` pour tout indice au-delà de la dernière variante connue, et la session **l'ignore** (elle continue) : une future version pourra ajouter des messages.

Messages ajoutés en fin d'énumération (indices figés 14 à 21) :

| # | Message | Rôle |
|---|---|---|
| 14 | `CollectionOffer(Vec<OfferedCollection>)` | Propose des collections : espace, partage, nom, nombre de fichiers, volume. |
| 15 | `CollectionReply { share, accepted }` | Réponse de l'utilisateur. |
| 16 | `Manifest { share, entries, last }` | Manifeste en parties de 500 lignes : chemin, empreinte (absente = suppression), taille, base. |
| 17 | `FileRequest { share, path, hash }` | Demande d'une version d'un fichier. |
| 18 | `FileChunk { share, path, hash, total, offset, data }` | Morceaux de 256 Kio, dans l'ordre. |
| 19 | `FileUnavailable { share, path, hash }` | La version demandée n'existe plus. |
| 20 | `FileAck { share, path, hash? }` | « J'ai maintenant cette version » (ou plus de fichier) : l'émetteur retient sa base. |
| 21 | `ShareStop { share }` | Arrêt du partage. |

**Déroulement.** Un partage ne concerne que les appareils qui **partagent déjà le profil** (espace commun) : l'offre voyage dans la session de ce profil, et la décision 7 est vérifiée par le moteur (appareils membres non révoqués) avant la création du partage. L'état d'un appareil pour un partage est `offered` puis `active` (côté qui propose : à la réponse ; côté qui reçoit : à l'acceptation).
1. Chaque côté envoie son manifeste au démarrage de la session (parcours complet, hachage seulement des fichiers modifiés), puis après chaque changement de ses fichiers (surveillance, regroupée 1 s) ou de ses transferts, **seulement s'il a changé** (anti-boucle).
2. À la réception d'un manifeste complet, chaque côté compare à ses fichiers (§12.2), retient les versions identiques, puis demande un par un les fichiers à recevoir ; un manifeste reçu pendant un échange est évalué après.
3. Chaque fichier reçu est vérifié (empreinte annoncée) ; une version demandée en même temps qu'elle est remplacée est servie depuis l'historique de la session (16 Mio, 64 versions), ce qui évite un faux conflit quand les deux côtés fusionnent ensemble.

### 12.4 Moteur (`adapter-sync::files`, sans réseau)

- **Écriture atomique** (`ShareDisk`, `adapter-fs`) : fichier temporaire voisin `.daftr-sync-*.tmp`, `sync_all`, renommage ; le temporaire disparaît en cas d'échec. Un `.tmp` n'est jamais un `.md`, donc jamais synchronisé.
- **Rien n'est écrasé sans l'avoir vu** : avant d'écrire, de supprimer ou de fusionner, le fichier local est relu et doit avoir l'empreinte que le plan a vue ; sinon la décision est abandonnée (le prochain manifeste la relance). Un échec sur un fichier (disque plein, droits, fichier ouvert) ne ferme pas la session.
- **Suppressions** : `trash` (corbeille du système, déjà utilisée par `adapter-fs`), sans confirmation.
- **Anti-boucle** : après chaque écriture, l'index et la base sont mis à jour avec l'empreinte écrite ; le parcours déclenché par la surveillance retrouve les mêmes empreintes et n'envoie rien.
- **Reprise après coupure** : au fichier près. Un fichier à moitié reçu est abandonné (rien n'est écrit) ; à la reconnexion, les manifestes sont échangés et les fichiers déjà écrits sont à jour. Pas de reprise à l'octet (10 Mio au plus par fichier).
- **Sécurité** : chemins distants validés (`SyncPath`) puis résolus par `resolve_inside` ; aucun lien symbolique suivi (lecture, écriture, descente dans un dossier) ; tailles bornées (morceaux, fichier, manifeste 40 000 lignes, offre 64 collections) ; noms de collection reçus assainis ; **rien n'est écrit avant l'acceptation** d'une collection proposée.
- **Fusion avec un dossier existant** : acceptée avec `merge`, sans base commune : identiques ignorés, absents copiés (dans les deux sens), différents gardés en double. Sans `merge`, un dossier non vide est refusé.

### 12.5 Intégration (`desktop`)

- **Surveillance** (`ShareWatcher`, `adapter-fs`) : un thread unique, **démarré avec le premier dossier partagé** (aucun thread sans partage), récursif, regroupe les rafales (400 ms) ; ignore les fichiers qui ne sont pas des `.md` et les temporaires de la synchronisation. Indépendante du profil actif et des collections dépliées.
- **Parcours complet** au démarrage (tous les profils synchronisés, actifs ou non), à la connexion d'un appareil, et à chaque partage créé ou accepté.
- Retirer une collection de la liste, ou changer son dossier, passe par le partage : `collections_remove` arrête d'abord la synchronisation ; `collections_relocate` et `collection_sync_relink` **oublient l'historique des échanges** du partage (sinon un dossier relié sur un autre contenu serait lu comme une suppression de masse).

Commandes (paramètres en camelCase côté renderer) :

| Commande | Paramètres | Rend |
|---|---|---|
| `collection_sync_state` | — | `CollectionSyncStateDto` (partages de tous les profils synchronisés, propositions en attente, `receivedFolder` résolu) |
| `collection_sync_summary` | `root` (collection du profil actif) | `FolderSummaryDto` (nombre et volume des `.md`, `hasGit`, fichiers écartés, `truncated`) |
| `collection_sync_inspect_folder` | `path` (absolu) | `FolderSummaryDto` (`exists`, `empty`, `mdFiles` : « ce dossier existe déjà (87 .md) ») |
| `collection_sync_share` | `root`, `deviceKeys[]` | `CollectionShareDto` ; les appareils doivent partager le profil actif |
| `collection_sync_stop` | `profileId`, `syncId` | — |
| `collection_sync_answer_offer` | `deviceKey`, `syncId`, `destination?`, `mode` : `receive` / `merge` / `decline` | — ; ensuite relire `collections_list` |
| `collection_sync_relink` | `root`, `folder?` (dialogue s'il est absent) | `CollectionDto?` (`null` : aucun dossier choisi) |
| `collection_conflict_resolve` | `root`, `path` (la copie), `keep` : `mine` / `other` | — ; la version gardée prend la place de l'original, la copie part à la corbeille |

Erreurs : refus `Collection already shared`, `Destination folder is not empty`, `Unknown collection share`, `Unknown collection` ; clés du moteur (`unknown_offer`, `untrusted`…) comme les commandes `sync_*`.

Événements :

| Événement | Charge utile | Quand |
|---|---|---|
| `sync:collection-offer` | `CollectionOffersDto` | Un appareil propose des collections (aussi relisibles par `collection_sync_state`) |
| `sync:collection-progress` | `CollectionProgressDto` (`done` / `total` par appareil) | Un échange de fichiers avance |
| `sync:collection-state` | `CollectionStateEventDto` (`share` absent : le partage n'existe plus) | État d'un partage changé : pastille, appareils, conflits, fichiers écartés |
| `sync:collection-conflicts` | `CollectionConflictsDto` | Les copies de conflit d'un partage ont changé (liste vide : tout est résolu) |

La pastille (`CollectionShareStatusKind`) est calculée côté backend, dans l'ordre : `missingFolder`, `syncing`, `conflicts`, `waiting` (aucun appareil actif connecté), `offered` (personne n'a encore accepté), `upToDate`. Une copie de conflit (`CollectionConflictDto`) donne son original, le nom de l'appareil dont elle garde la version, et `mine`.

### 12.6 Écarts par rapport à [collections.md](collections.md)

- **Un watcher dédié** (`ShareWatcher`) et non « le watcher existant des collections » : celui-ci ne surveille que les collections dépliées du profil actif et pousse des arbres vers l'interface ; le partage doit suivre tous les dossiers partagés de tous les profils.
- **Aucune table de « conflits »** : une copie de conflit est reconnue à son nom (`nom (conflit X).md` à côté de `nom.md`) ; le résumé du partage les liste.
- **Le suiveur fait la copie, l'arbitre ne fait rien** : la copie porte le nom de l'appareil qui l'a créée (le suiveur), pas celui de « l'autre » vu de chaque côté.
- **Une proposition refusée n'est pas mémorisée** : si l'appareil qui propose est connecté, son partage se retire ; sinon il reproposera à la prochaine connexion. Les propositions en attente sont gardées en mémoire (perdues au redémarrage), mais le proposant les renvoie tant qu'elles n'ont pas de réponse.
- **Reprise au fichier, pas à l'octet.**
- **Texte de la base limité à 256 Kio** : au-delà, deux versions différentes sont gardées en double plutôt que fusionnées.
- Les réponses « Plus tard » ne passent pas par le backend (la proposition reste dans `collection_sync_state`).
- Arrêter ou retirer un appareil du profil (`sync_stop_sharing`, `sync_remove_device`) retire l'appareil de ses collections partagées (un partage sans appareil disparaît) ; aucun événement `sync:collection-state` n'est émis dans ce cas : le renderer relit l'état.

### 12.7 Limites et points non vérifiés

- Un dossier vidé d'un coup (disque ou partage réseau démonté, dossier de montage vide) est lu comme des suppressions et part à la corbeille chez l'autre appareil (récupérable). Un dossier qui n'existe plus n'envoie rien et ne supprime rien (`missingFolder`).
- Une base ou un accusé perdu n'a jamais d'effet destructeur : au pire une copie de conflit en trop.
- Les fichiers ouverts en écriture exclusive par un autre programme (renommage refusé) sont repris au prochain manifeste.
- **Non vérifié** : deux PC réels, la corbeille de Windows et de macOS par `ShareDisk::trash` (testée par le moteur sur un disque en mémoire seulement), la surveillance du système sur de gros dépôts (checkout git, 20 000 fichiers), les liens symboliques sous Windows (testés sous Unix seulement), les dossiers réseau et OneDrive, l'interface (voir §12.8 : les écrans sont livrés, mais jamais lancés dans l'application réelle).

### 12.8 Interface des collections partagées (renderer)

L'état vient de `CollectionSyncService` (`core/services/collection-sync.service.ts`), qui lit `collection_sync_state` au démarrage et suit les quatre événements `sync:collection-*`. Les règles pures (pastille et infobulle, tri des propositions, fusion des événements dans l'état, copie de conflit d'un fichier, mode de réponse selon le dossier, tailles et durées) sont dans `core/utils/collection-sync.util.ts` ; la comparaison ligne à ligne dans `core/utils/line-diff.util.ts`. Tests : `tests/collection-sync-util.test.ts`, `tests/line-diff.test.ts`.

- **Menu d'une collection** (barre latérale) : « Synchroniser avec mes appareils… » (ouvre l'assistant), ou « Arrêter de synchroniser cette collection » (confirmation) quand elle est partagée ; « Relier à un autre dossier… » (`collection_sync_relink`) remplace « Localiser sur le disque » pour une collection partagée dont le dossier manque.
- **Assistant** (`collection-share-dialog`) : appareils qui partagent le profil actif en cases à cocher (tous cochés), résumé (`collection_sync_summary` : nombre, volume, fichiers de plus de 10 Mio ou écartés, parcours tronqué), avertissement git, et « Liez d'abord un appareil » avec un bouton qui ouvre « Appareils et synchronisations » du profil actif (`ProfilesService.requestDevices`, relayé par le sélecteur de profil). Ouvert depuis la vue du profil, il demande d'abord la collection (les dossiers pas encore partagés).
- **Proposition reçue** : un toast (« Voir ») par événement contenant des propositions pas encore annoncées, puis la fenêtre (`collection-offer-dialog`) : destination pré-remplie, « Modifier… » (dialogue natif), avertissement « dossier existant (N .md) » avec « Fusionner » présélectionné ou « Choisir un autre dossier » (ouvre le dialogue). Accepter envoie `merge` si le dossier existe et n'est pas vide, `receive` sinon ; Ignorer envoie `decline` ; **Plus tard** ferme la fenêtre, la proposition reste dans `offers`.
- **Propositions en attente** (« Plus tard », ou relues après un redémarrage) : une ligne « N collections proposées · Répondre » en tête de la section Collections de la barre latérale, et la liste « Collections proposées » de la vue du profil.
- **Pastille** sur la ligne d'une collection partagée, avant le « N » existant : ⟳ à jour (infobulle « Synchronisée avec X, il y a 2 min »), ⟳ animé avec « 37 / 142 » en cours, ⚠ conflits, ⏸ « En attente de X », ✕ « Dossier introuvable », horloge « Proposée à X ». Un clic relie le dossier s'il manque, sinon ouvre la vue « Appareils et synchronisations ».
- **Vue du profil** : section « Collections partagées » (état, appareils, « Relier… » si le dossier manque, « Arrêter » avec confirmation, « Ajouter une collection » pour le profil actif seulement).
- **Réglage** : « Dossier des collections reçues » (chemin + « Parcourir… ») dans Réglages → Synchronisation ; `SettingsDto.receivedCollectionsFolder` est repris dans `settings.model.ts`. La réinitialisation générique du réglage vide le chemin.
- **Conflits** : ⚠ sur la copie dans l'arbre (et sur les dossiers qui en contiennent) ; au-dessus de l'éditeur, un bandeau « Garder ma version » / « Garder l'autre » / « Comparer » (`collection_conflict_resolve`, puis l'onglet de la copie se ferme) ; « Comparer » affiche l'original et la copie côte à côte, lignes différentes surlignées.
- Retirer un appareil ou arrêter un profil (`sync_remove_device`, `sync_stop_sharing`) relit l'état des collections (`SyncService.membershipChanges`).

Écarts par rapport à [collections.md](collections.md) :

- **Une commande Rust ajoutée** : `collection_sync_pick_folder` (dialogue natif de dossier, `None` si annulé). Le renderer n'avait aucun moyen d'ouvrir un sélecteur de dossier hors des commandes de collections (« Modifier… », « Parcourir… »).
- Le dossier choisi par « Modifier… » est le dossier de destination lui-même, pas un parent : ce qu'on voit est ce qui est écrit.
- Un clic sur la pastille ouvre la vue « Appareils et synchronisations » du profil (qui ouvre aussi le mode d'appairage) : il n'y a pas d'écran de détail propre à une collection.
- « Comparer » est une comparaison ligne à ligne côte à côte (`line-diff.util.ts`, table limitée à 4 millions de cellules, au-delà le bloc différent est montré d'un seul tenant) ; `text-diff.util.ts` calcule un changement minimal pour l'éditeur, il ne convient pas à l'affichage.
- Le bandeau de conflit n'est pas dans l'aperçu à part : il est au-dessus de l'éditeur et de l'aperçu (les deux modes de la même vue).
- Pas de test de composant (le projet n'en a pas) : seules les règles pures sont testées.

**Non vérifié** : tous les écrans ci-dessus n'ont jamais été lancés dans l'application réelle (build, lint et tests vitest seulement) : mise en page des pastilles et du dialogue, dialogue natif de dossier, ordre des dialogues au-dessus de la fenêtre des réglages du profil, Échap quand plusieurs fenêtres sont ouvertes, événements de progression réels.

## 13. Phases ultérieures

Client mobile (scan du QR code, même format), scan du QR par webcam sur desktop, export / import d'un paquet de modifications par fichier pour les appareils jamais présents ensemble, historique des versions, `y-codemirror.next` (Yjs dans l'éditeur, ce qui rendrait la fusion à trois voies inutile), offres persistées, mDNS allégé pour revenir sous 1 Mo, keepalive.
