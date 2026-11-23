# Synchronisation LAN : avancement des lots

> Document de travail pour les lots restants : ce que chaque lot a livré et ce que les suivants doivent appeler ou implémenter. Spec : [conception-lan.md](conception-lan.md). Le lot 6 le fusionnera dans [plan.md](plan.md).

Toute la feature vit dans la branche `NF/feat/sync-lan-261123` (une seule PR vers `recipe`).

| Lot | État | Historique |
|---|---|---|
| 0 — spike | Fait, non livré | PR #86 fermée, mesures dans [mesures-spike.md](mesures-spike.md) |
| 1 — modèle | Fait | fusionné (ex-#89) |
| 2 — CRDT pur | Fait | fusionné (ex-#85) |
| 3 — protocole | Fait | fusionné (ex-#87) |
| 4 — intégration | À faire | — |
| 5 — front | À faire | — |
| 6 — doc | À faire | — |

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
