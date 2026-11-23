# Synchronisation LAN : avancement des lots

> Document de travail pour les lots restants : ce que chaque lot a livré et ce que les suivants doivent appeler ou implémenter. Spec : [conception-lan.md](conception-lan.md). Le lot 6 le fusionnera dans [plan.md](plan.md).

Toute la feature vit dans la branche `NF/feat/sync-lan-261123` (une seule PR vers `recipe`).

| Lot | État | Historique |
|---|---|---|
| 0 — spike | Fait, non livré | PR #86 fermée, mesures dans [mesures-spike.md](mesures-spike.md) |
| 1 — modèle | Fait | fusionné (ex-#89) |
| 2 — CRDT pur | Fait | fusionné (ex-#85) |
| 3 — protocole | Fait | fusionné (ex-#87) |
| 4 — intégration | Fait | commits directs sur la branche (PR #90) |
| 5 — front | À faire | — |
| 6 — doc | À faire | — |

## Lot 4 — intégration (`desktop`, `adapter-fs`, `presentation`)

**Ce qui est branché**
- `adapter-fs` : [`TrustFiles`](../../src-tauri/crates/adapter-fs/src/sync_files.rs) (`impl TrustStore`) et `SpaceIndexFile`, dans `<données>/sync/` : `device.key` (32 octets bruts ; `0600` sous Unix, ACL « utilisateur seul » par `icacls` sous Windows, repli sur les ACL de `%APPDATA%` en cas d'échec), `devices.json`, `index.json`. Écriture atomique (fichier voisin + renommage) ; lecture tolérante (JSON illisible : liste vide + copie `.corrupted` ; clé illisible : mise de côté, nouvelle identité ; entrée invalide ignorée ; une révocation n'est jamais perdue).
- `adapter-sqlite` : `read_stored_sync(path)` lit l'espace d'une base en lecture seule, sans migration ni création (reconstruction du cache au démarrage).
- `adapter-sync` : `Engine::set_port` (l'écoute se rouvre à chaud) et `local_addresses()`.
- `desktop/src/sync/` : [`SyncState`](../../src-tauri/crates/desktop/src/sync.rs) (dans `DesktopState`), moteur unique créé au démarrage sur le runtime tokio de Tauri avec `load_or_create_identity`, `MdnsDiscovery(AddrPolicy::Lan)`, `SystemClock`. [`DesktopSyncProfiles`](../../src-tauri/crates/desktop/src/sync/profiles.rs) (`impl SyncProfiles`, `Profile = ProfileId`) : toute base de profil (actif, inactifs, cible d'un transfert, profil créé) s'ouvre par `open_store` avec `with_device` et un écouteur qui appelle `engine.local_change(profil)` (MCP et overlay compris : ils écrivent dans la base active) ; le profil actif passe par sa base ouverte, les autres (verrouillés compris) sont ouverts le temps d'un accès puis refermés ; cache `space_id → profil` en mémoire, recopié dans `index.json`, reconstruit au démarrage et remis à jour après chaque accès du moteur.
- Démarrage : relecture des espaces, puis `set_paused(true)` si la pause est réglée, sinon `refresh()`. `refresh()` aussi après création d'espace, (dés)activation, suppression d'un profil. `shutdown()` à `RunEvent::Exit`.
- Re-diffusion A→B→C : déjà faite par `adapter-sync` (après `apply_remote`, la session pousse aux autres sessions, sauf celle d'origine) ; vérifiée de bout en bout.
- Changement de réseau : relecture des adresses locales toutes les 30 s, seulement quand la synchronisation tourne (ni désactivée, ni en pause) → `network_changed()`. Les API natives de notification demandent du code `unsafe` (interdit) ou une dépendance lourde.
- Réglages de l'appareil (`domain::settings`) : `syncDeviceName` (vide = nom d'hôte : `COMPUTERNAME`, `scutil --get ComputerName`, `/etc/hostname`), `syncPort` (47820, 1024–65535), `syncPaused` ; appliqués à chaud (`SettingsChange::sync_changed`), aussi par `settings_set`.
- i18n backend : `SyncDefaultDeviceName` (nom d'hôte introuvable), `SyncDefaultSpaceName` (profil sans nom proposé). Pas de notification système (pas de plugin de notification) : le lot 5 notifie dans l'interface.
- Plateformes : hooks NSIS (`desktop/windows/hooks.nsh`) : règles entrantes TCP 47820 et UDP 5353, profil Privé, limitées à `Daftr.exe`, via `netsh` élevé (une invite UAC, une seule fois : marqueur `HKCU\Software\Daftr\Sync`), supprimées à la désinstallation (pas lors d'une mise à jour `/UPDATE`). macOS : `desktop/Info.plist` (fusionné par Tauri) avec `NSLocalNetworkUsageDescription` et `NSBonjourServices` (`_daftr-sync._tcp`).
- Tests : fichiers `sync/` (atomicité, lecture corrompue), `SyncProfiles` (base active partagée, inactive ouverte puis fermée, verrouillé synchronisé, offre « nouveau profil » refusée si l'espace est déjà là, cache reconstruit), démarrage sans rien qui tourne (critère 8) et pause, et [bout en bout](../../src-tauri/crates/desktop/src/sync/tests/end_to_end.rs) : trois installations (dossiers, vraies bases, moteurs sur loopback) ; A↔B par code → synchro complète immédiate ; modification sur A visible sur B dont le profil synchronisé n'est pas actif ; C lié au seul B reçoit la modification suivante de A par B (~1 s).

**API pour le lot 5** (commandes dans [`commands/sync.rs`](../../src-tauri/crates/desktop/src/commands/sync.rs), DTO dans [`presentation/src/dto/sync.rs`](../../src-tauri/crates/presentation/src/dto/sync.rs), bindings dans `frontend/src/app/core/bindings/`)

| Commande | Paramètres | Rend |
|---|---|---|
| `sync_state` | — | `SyncStateDto` (statut, clé et nom de l'appareil, nom par défaut ?, port, port d'écoute, pause, profils synchronisés) |
| `sync_devices` | — | `TrustedDeviceDto[]` (nom, origine, ajout, retrait, présent, connecté, profils partagés avec `lastSyncAt`) |
| `sync_profile` | `profileId` | `ProfileSyncDto` (espace, activation, membres `SpaceDeviceDto`) |
| `sync_start_pairing` | `profileIds[]` | `PairingTicketDto` (code, uri, `qrSvg`, `expiresAt` en ms Unix) ; crée les espaces manquants |
| `sync_stop_pairing` | — | — |
| `sync_pair` | `code`, `address?` (`IP:port`, `[IPv6]:port` ou IP seule) | `LinkedDeviceDto` |
| `sync_answer_offer` | `deviceKey`, `spaceId`, `choice` (`new` / `merge` / `ignore`), `profileId?` (avec `merge`) | — |
| `sync_offer_profile` | `profileId`, `deviceKey` | — (crée l'espace au besoin) |
| `sync_stop_sharing` | `profileId`, `deviceKey` | — |
| `sync_remove_device` | `deviceKey` | — |
| `sync_set_profile_enabled` | `profileId`, `enabled` | — (activer crée l'espace au besoin) |
| `sync_now` | — | — |
| `sync_set_paused` | `paused` | `SyncStateDto` |
| `sync_set_device_name` | `name` (vide = nom d'hôte) | `SyncStateDto` |
| `sync_set_port` | `port` | `SyncStateDto` |

Erreurs : `IpcError` ; les échecs du moteur ont le code `sync` et une clé stable en message (`wrong_code`, `not_pairing`, `no_pairing_device`, `unreachable`, `timeout`, `not_local`, `listen_failed`, `discovery_failed`, `untrusted`, `unknown_offer`, `paused`, `fingerprint_mismatch`, `invalid_qr`, `protocol_version`, `protocol_failed`, `stopped`) ; « nouveau profil » sur un espace déjà présent : `refused`, message `profile already synchronized`.

| Événement | Charge utile |
|---|---|
| `sync:status` | `SyncStatusDto` (`state` : `disabled` / `paused` / `upToDate` / `syncing` / `noDevice` / `error`, `devices`, `reason`) |
| `sync:notes-synced` | `NotesSyncedDto` (`profileId`, `active`, `uuids`, `folders`) ; pour le profil actif, `notes:changed` est aussi émis |
| `sync:progress` | `SyncProgressDto` (`profileId`, `done`, `total`) |
| `sync:initial-done` | `InitialSyncDoneDto` (`profileId`) |
| `sync:device-linked` | `LinkedDeviceDto` |
| `sync:offer-received` | `OfferReceivedDto` (`device`, `offers[]` : `spaceId`, `name`, `noteCount`) |
| `sync:pairing` | `PairingStateDto` (`active`, `ticket`) : nouveau code après 10 échecs, ou fin du mode |

Réglages : `SettingsDto` gagne `syncDeviceName`, `syncPort`, `syncPaused` (modèle du front mis à jour).

**Écarts avec la spec**
- Les événements sont préfixés `sync:` comme les autres événements de l'app (`sync:notes-synced` pour `notes-synced`).
- Un profil créé par une offre n'émet pas d'événement de liste des profils : le front relit `profiles_state` après `sync:initial-done` ou `sync:offer-received`.
- Le nom proposé d'un profil sans nom est « Mes notes » / « My notes » (langue des réglages).
- `device.key` est créé au premier lancement même sans synchronisation (toutes les bases s'ouvrent avec la clé) ; c'est la seule trace d'une installation sans profil synchronisé (pas d'`index.json`, pas d'écoute, pas de thread).
- Identité illisible et impossible à recréer : identité de session (journalisée), les autres appareils ne la reconnaîtront pas.

**Points d'attention**
- 🟠 Installation par utilisateur : la règle de pare-feu demande une élévation UAC à l'installation (une fois). Refusée, Windows pose sa propre question à la première écoute. Un port changé dans les réglages n'est pas couvert par la règle.
- 🟠 Les profils inactifs sont rouverts à chaque accès du moteur (migration vérifiée et purge à l'ouverture) : coût à mesurer sur de gros profils pendant une synchro initiale.
- 🟠 Changer le port pendant un appairage laisse l'ancien port dans le QR code (le code reste valable via mDNS).
- Non vérifié : deux PC réels (critères 1–7), invite et règles du pare-feu Windows (le hook est seulement compilé avec `makensis`), macOS (`Info.plist`, invite « réseau local »), traduction de `NSLocalNetworkUsageDescription` (anglais seul).
- Linux (lot 6, doc) : si ufw ou firewalld est actif, ouvrir le port TCP de la synchronisation (47820 par défaut) et l'UDP 5353 entrant (`ufw allow 47820/tcp`, `ufw allow 5353/udp` ; `firewall-cmd --add-port=47820/tcp --add-service=mdns --permanent`).

## Lot 2 — CRDT pur (`app::sync`, ports)

**domain** (sans dépendance)
- `domain::uuid::Uuid` : `from_random([u8;16])`, `derived(ns, name)` (UUIDv5, SHA-1 maison), `parse`, `Display`, `Ord`.
- `domain::sync::{SpaceId, DeviceKey}` : 32 octets, `from_bytes`, `as_bytes`, `parse` (64 hex), `Display`.
- `DocId::{Space, Note(Uuid)}` ; `DeviceName::{parse, sanitize}` ; `PairingCode::{parse, from_number(u32)}` (le `Debug` masque le code).
- `default_folder_uuid(&SpaceId)`, `purge_cutoff`, `is_purgeable`, `is_timestamp`, `timestamp_from_unix`.

**app::sync**
- `trait SyncDoc` (`NoteDoc`, `SpaceDoc`) : `encode`, `state_vector`, `diff`, `apply_update`, `has_missing_updates`, `own_clock`.
- `StateVector`, `Update`, `MAX_UPDATE_BYTES` = 16 Mio ; `SyncError::{TooLarge, Malformed, MalformedStateVector, Rejected, AlreadySynced}`.
- `note_doc` : `from_note`, `apply_changes`, `apply_placement`, `apply_color`, `apply_delete`, `apply_restore`, `project -> ProjectedNote`.
- `space_doc` : `create`, `rename`, `put_folder`, `delete_folder`, `empty_folder`, `add_device`, `rename_device`, `revoke_device`, `repair_cycles`, `project -> ProjectedSpace` (`is_member`).
- `merge::plan(&MergeInput) -> MergePlan`, `publish_local_folders`.
- Feature `testing` : `MemorySyncStore`, `MemoryProfiles`, `RecordedEvents`.

**Ports** (`app::ports::sync`, `app::ports::trust`)
- `SyncStore` : `space_id`, `changed_since`, `state_vector`, `diff`, `apply_remote -> Applied{notes, space, rejected, incomplete}`, `peer` / `set_peer`, `devices`, `adopt_space(&SpaceId, &RemoteSnapshot{space, notes})`, `add_device`, `revoke_device`, `offer`.
- `SyncProfiles { type Profile; profile_for; with_store; create_profile; synced_profiles }`.
- `SyncEvents { type Profile; publish(SyncEvent) }` : `NotesSynced`, `Status`, `Progress`, `InitialSyncDone`, `DeviceLinked`, `OfferReceived`.
- `TrustStore { device_secret, save_device_secret, devices, put_device }`, `TrustedDevice{key, name, origin: Paired | Space, added_at, revoked_at}` ; doublure `MemoryTrust`.

Écarts avec la spec : `updated_at` et une racine `edits` (horloge de la dernière modification par client) ajoutés aux documents de note, pour une projection déterministe et la règle suppression contre modification ; `adopt_space` reçoit aussi les notes distantes ; cycles réparés à la projection sans écriture ; identifiant client Yjs = 53 bits de poids faible des 8 premiers octets de la clé publique.

## Lot 3 — protocole (crate `adapter-sync`)

**Démarrage (lot 4)**
- `load_or_create_identity(&trust)` donne la clé de l'appareil, qui sert aussi à ouvrir les stores.
- `Engine::new(identity, EngineConfig::new(nom), EngineDeps { profiles, events, trust, discovery: Arc::new(MdnsDiscovery::new(AddrPolicy::Lan)), clock: Arc::new(SystemClock), runtime: tokio::runtime::Handle })`.

**À appeler**
- `refresh()` au démarrage, après création d'un espace, activation ou désactivation d'un profil ; `local_change(&profil)` après chaque sauvegarde ; `network_changed()` ; `set_paused`, `sync_now`, `set_device_name`.
- Appairage : créer l'espace des profils cochés **avant** `start_pairing(profils) -> PairingTicket{code, PairingUri, qr_svg, expires_at}` ; `pairing()` (watch : nouveau code, fin du mode) ; `stop_pairing`, `pair_with_code(code, Option<SocketAddr>)`, `pair_with_qr(uri)`.
- `offer(profil, device)`, `answer_offer(device, space, OfferChoice::{NewProfile, Merge(p), Ignore})`, `stop_sharing`, `remove_device`.
- `status()`, `connected_devices()`, `present_devices()`, `listening_port()`, `shutdown()`.
- Erreurs : `refresh()` renvoie `Error::Listen` ; statut `SyncStatus::Error{reason: "listen_failed" | "discovery_failed"}` ; `Error::status_reason()` donne des clés stables pour l'indicateur.

Écarts avec la spec : le temps réel relance un rattrapage ciblé (`Changed` / `Diffs` / `Updates` / `Done` / `Ack`) au lieu d'un `Update` direct ; une offre acceptée envoie un `Snapshot` puis `adopt_space` ; offres en attente gardées en mémoire (perdues au redémarrage) ; mode appairage fermé à la première liaison ; `last_addr` enregistrée seulement pour les connexions sortantes ; pas de keepalive ; dépendance `if-addrs` (déjà tirée par `mdns-sd`).

Points d'attention : SPAKE2 en variante magic-wormhole (pas RFC 9382), construction documentée dans `pake.rs` pour le futur client mobile ; un appareil sans profil synchronisé n'écoute pas et ne reçoit une offre que s'il compose ; backoff limité à `last_addr` ; interfaces virtuelles écartées par heuristique de noms.

## Lot 1 — modèle (`adapter-sqlite`)

**API pour le lot 4**
- Ouvrir un store de profil : `SqliteStore::open(path)?.with_device(key)`. La clé est obligatoire pour écrire dans un profil synchronisé : **toutes** les bases de profil (y compris la cible d'un transfert) doivent être ouvertes avec `with_device`.
- `.with_change_listener(Arc<dyn Fn()>)` : appelé après chaque écriture locale qui a modifié un document (MCP compris) → brancher `engine.local_change(profil)`.
- `create_space(name, &DeviceName) -> SpaceId` : tire le `space_id`, crée le document `space` et `sync_state`, donne l'uuid dérivé à l'Inbox, construit les blobs de toutes les notes.
- `sync_enabled()` / `set_sync_enabled(bool)`.
- `SyncStore` complet, `apply_remote` par lots de 200 par transaction. `apply_remote` ne prévient **pas** le change listener : la re-diffusion A→B→C en temps réel est au moteur.

Détails : migration `add_sync_columns` (sauvegarde `.pre-sync` par `VACUUM INTO`) ; FTS limitée aux notes visibles ; soft delete filtré partout ; `domain::links::{rewrite_note_links, retarget_note_links}` ; le front et le MCP écrivent des `note://<uuid>` ; transfert entre profils = nouveaux uuid côté cible, liens entre notes copiées suivis ; identifiant client Yjs = clé de l'appareil pour la création ou la fusion d'un espace, puis génération aléatoire par session d'écriture (protection contre la restauration d'une sauvegarde) ; purge à 30 jours à l'ouverture de la base ; blobs construits à `create_space` / `adopt_space` puis tenus à jour.
