# Conception : synchronisation entre appareils d'un même utilisateur

> Spec d'origine de la synchronisation LAN (2026-11-19), gardée telle qu'écrite avant l'implémentation. La feature livrée (architecture, protocole, API, écarts assumés, limites) est décrite dans [plan.md](plan.md), les mesures du spike dans [mesures-spike.md](mesures-spike.md).

## 0. Contexte et décisions

- **Remplace l'ancien plan** (serveur OVH, OAuth). On garde l'idée de sa phase 1 (modèle de données CRDT), adaptée au §3. Les phases serveur, compte et exploitation sont abandonnées.
- **Réseau local uniquement.** Aucun trafic Internet, ni direct ni via une infrastructure tierce. L'application refuse toute connexion de synchronisation vers ou depuis une adresse qui n'est pas locale. Deux appareils ne se synchronisent que lorsqu'ils sont allumés en même temps sur le même réseau local.
- **Deux notions distinctes** :
  - **la confiance entre appareils**, établie une fois par appairage (QR code ou code à 6 chiffres) ;
  - **les profils synchronisés**, choisis profil par profil pour chaque appareil.
- **Tous les profils synchronisés se synchronisent, actifs ou non.**
- **Automatique et en temps réel**, avec une pause globale et un bouton « Synchroniser maintenant ».
- **Synchronisation immédiate** après chaque appairage et chaque ajout de profil.
- **Pas de dialogue de conflit** : la fusion CRDT est automatique, caractère par caractère. On affiche seulement un indicateur discret quand une note ouverte a reçu des modifications distantes.
- **Hors périmètre** : partage avec d'autres personnes, synchronisation des réglages, synchronisation des collections (chemins disque propres à chaque machine), application mobile, historique des versions.
- **Contraintes** : coût en binaire, CPU et RAM mesuré et borné (§10). Rien ne tourne quand aucun profil n'est synchronisé ou quand la synchronisation est en pause.

## 1. Choix techniques

| Sujet | Choix | Raison |
|---|---|---|
| CRDT | `yrs`, avec `OffsetKind::Utf16` | Format binaire Yjs : `y-codemirror.next` pourra se brancher plus tard |
| Diff texte → opérations CRDT | `similar` | L'éditeur continue d'envoyer le contenu complet ; pas de Yjs côté front en v1 |
| Transport | TCP + Noise `XX_25519_ChaChaPoly_BLAKE2s` (`snow`), trames préfixées par leur longueur, contenu `postcard` | Chiffrement et authentification mutuelle par clé statique, empreinte minimale, aucun code Internet embarqué |
| Vérification du code d'appairage | PAKE SPAKE2 (`spake2`), lié au hash du handshake Noise | Un code à 6 chiffres reste sûr : un seul essai en ligne par tentative, rien à rejouer hors ligne, pas d'interception possible |
| Découverte | `mdns-sd` (Rust pur, sans Avahi ni Bonjour système), service `_daftr-sync._tcp.local` | Aucune infrastructure |
| QR code | `qrcode` (rendu SVG, sans dépendance image) | Léger |
| Repli | Saisie manuelle `IP:port` | Réseaux où le mDNS est filtré (isolation des clients Wi-Fi, certains réseaux d'entreprise) |

## 2. Identité, confiance et profils

**Fichiers de l'appareil** (dans `<data root>/sync/`, communs à tous les profils, écrits de façon atomique)
- `device.key` : clé statique x25519 Noise, permissions restreintes. L'identifiant de l'appareil est sa clé publique.
- `devices.json` : **appareils de confiance**. Pour chacun : clé, nom, origine (`paired` = appairé directement, `space` = connu via un profil commun), date d'ajout, `revoked_at`.
- `index.json` : correspondance `space_id` → `ProfileId`. C'est un cache, reconstruit au démarrage en lisant `sync_state` de chaque profil.

**Profils** (une base par profil, `profiles/<id>/daftr.db`, le profil `default` à la racine)
- Un profil synchronisé possède un **espace** (`space_id`, 32 octets aléatoires) et appartient à **un seul espace**.
- **Le `ProfileId` est local** et n'est jamais échangé : le profil `work` du desktop peut correspondre au profil `pro` du laptop.
- La configuration propre au profil (espace, activation) est dans `sync_state`, dans la base du profil. Elle suit le dossier et disparaît avec lui.
- **Tous les profils synchronisés tournent, actifs ou non.**
  - Pour un profil inactif, la base est ouverte le temps d'appliquer ou de servir des changements, puis refermée.
  - Pour le profil actif, on passe par l'instance déjà ouverte, pour que l'interface reçoive les événements.
- **Profils protégés par mot de passe** : la base n'est pas chiffrée (le mot de passe verrouille seulement l'accès dans l'app), donc ils se synchronisent même verrouillés. Si la base est un jour chiffrée, ces profils attendront d'être déverrouillés.

**Règle d'accès à un espace** : un appareil est accepté pour un espace S si sa clé figure dans `S.devices` et n'y est pas révoquée. La confiance est donc transitive au sein d'un espace : un appareil ajouté à S par un membre peut se synchroniser avec tous les autres membres, sans appairage croisé.

**Réglages propres à l'appareil**, dans le fichier de réglages commun (`src-tauri/crates/domain/src/settings.rs`) : nom de l'appareil (hostname par défaut), port d'écoute (47820 par défaut), pause globale.

## 3. Modèle de données (base de chaque profil)

**Migration idempotente `add_sync_columns`.** Elle sauvegarde d'abord `daftr.db` en `daftr.db.pre-sync` si ce fichier n'existe pas encore.
- `notes` : `uuid TEXT` (index unique), `deleted_at TEXT NULL`, `crdt BLOB NULL`, `seq INTEGER NOT NULL DEFAULT 0`, `folder_uuid TEXT NULL`.
- `folders` : `uuid TEXT` (index unique), `deleted_at TEXT NULL`.
- `sync_docs (doc_id TEXT PRIMARY KEY, crdt BLOB NOT NULL, seq INTEGER NOT NULL)` : contient le document `space`.
- `sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL)` : `space_id`, `enabled`, `next_seq`.
- `sync_peers (device_key TEXT PRIMARY KEY, acked_seq INTEGER NOT NULL DEFAULT 0, last_addr TEXT, last_seen_at TEXT)` : curseur et dernière adresse connue de chaque appareil distant pour ce profil.

**Règles**
- **`seq` est un compteur local monotone**, attribué par l'adaptateur à **chaque écriture de blob**, qu'elle soit locale **ou distante**. Ce second cas permet la propagation transitive A→B→C. Pas de triggers : tout passe par l'adaptateur, qui réécrit le blob dans la même transaction que les colonnes.
- **Soft delete** : `NoteStore::delete` écrit `deleted_at`. Les lectures (`all`, `by_id`, `search`, `by_tag`, `tag_columns`) filtrent `deleted_at IS NULL`, et la FTS retire les notes supprimées.
- **`id INTEGER` reste la poignée locale** pour le front, les onglets et le MCP. Une note reçue d'un autre appareil reçoit un nouvel `id` local. Le domaine expose en plus `uuid` (exports `ts-rs` régénérés).
- **Liens `note://<entier>` → `note://<uuid>`** :
  - backend : fonction pure `domain::links::rewrite_note_links`, appliquée en une transaction, idempotente ;
  - front : la regex `NOTE_LINK` accepte `\d+|<uuid>`, et les résolutions passent par une `Map<uuid, Note>` ;
  - MCP : les outils écrivent des uuid.
- **Dossier par défaut** : son uuid est dérivé de l'espace, `UUIDv5(space_id, "default-folder")`. Il est réécrit à la création ou à la jonction d'un espace, pour éviter deux « Inbox » après la fusion.

**Documents CRDT**
- `note:<uuid>` : `content` (`Y.Text`), `meta` (`Y.Map` : `tags`, `folder` (uuid), `sort_order`, `color`, `created_at`, `deleted_at`, `deleted_sv`). Le titre reste dérivé de la première ligne.
- `space` : `name` (nom proposé du profil), `folders` (`Y.Map` uuid → `{name, parent, sort_order, color, is_default, deleted_at}`) et `devices` (`Y.Map` clé publique → `{name, added_at, revoked_at}`).
- Le blob est toujours ré-encodé depuis l'état courant (`encode_state_as_update_v1`), avec le GC de `yrs` actif : pas d'historique qui gonfle. Le blob d'une note pas encore migrée est construit à la volée.

## 4. Règles de fusion et de projection (module pur `app::sync`)

Ce sont des fonctions sans I/O : `note_doc::{from_note, apply_changes, apply_placement, apply_color, apply_delete, project}` et `space_doc::{…, repair_cycles, project}`. **La projection est déterministe** : deux appareils dans le même état CRDT produisent les mêmes lignes SQLite, sans aucune écriture supplémentaire. L'identifiant client Yjs est dérivé de la clé de l'appareil.

1. **Texte** : fusion `Y.Text` native.
2. **Métadonnées** : last-writer-wins de `Y.Map`. Égalité de `sort_order` : départage par uuid.
3. **Suppression contre modification concurrente** : `apply_delete` écrit `deleted_at` et `deleted_sv` (le state vector au moment de la suppression). À la projection, la note est supprimée **seulement si** aucune opération de contenu ne dépasse `deleted_sv`. Sinon elle reste visible : une modification faite ailleurs, sans connaître la suppression, restaure la note.
4. **Dossier inconnu ou supprimé** : la note est projetée à la racine (`folder_id` NULL) et garde son `folder_uuid`. Les notes non résolues sont reprojetées après chaque application du document `space`.
5. **Cycles de dossiers** (déplacements concurrents) : le dossier au plus petit uuid du cycle est rattaché à la racine.
6. **Suppression d'un dossier** : reproduire le comportement actuel (`adapter-sqlite/src/folders.rs`) sous forme de tombstones dans les documents concernés.
7. **Purge** : après 30 jours, le contenu d'une note supprimée est vidé, mais la ligne tombstone (uuid, `deleted_at`) est gardée. Si une mise à jour arrive plus tard pour cet uuid, on recrée la note : la perte de données est pire qu'un doublon.
8. **Fusion d'un profil existant dans un espace** : seulement pour un profil qui n'est **pas déjà synchronisé** (un profil appartient à un seul espace).
   - Avant la première synchronisation, on sauvegarde la base en `daftr.db.pre-merge-<date>`.
   - On reçoit d'abord le document `space` distant.
   - Chaque dossier local dont le chemin (chaîne des noms depuis la racine) existe à distance adopte l'uuid distant, et ses notes suivent.
   - Le dossier par défaut adopte l'uuid dérivé.
   - Les notes de contenu identique dans le même chemin de dossier sont dédoublonnées (la locale adopte l'uuid distant). Toutes les autres sont conservées.

## 5. Architecture

```
domain        : types (SpaceId, DeviceKey, DeviceName, PairingCode), sans dépendance
app::sync     : CRDT pur (§4) + ports SyncStore, SyncProfiles, SyncEvents
adapter-sqlite: impl SyncStore (blobs, seq, curseurs, projection en transaction)
adapter-fs    : device.key, devices.json, index.json (écriture atomique)
adapter-sync  : NOUVELLE crate — écoute TCP, Noise, PAKE, mDNS, appairage, moteur (§6, §7), sans SQL
desktop       : moteur unique par installation, impl SyncProfiles (ouvre la base d'un profil, réutilise celle du profil actif), commandes Tauri, événements
frontend      : Réglages → Synchronisation, réglages du profil, indicateur, écrans d'appairage, application des mises à jour distantes
```

Ports (indicatifs) :
- **`SyncStore`**, pour un profil :
  - `changed_since(seq) -> Vec<(DocId, StateVector, seq)>`, `state_vector(doc)`, `diff(doc, remote_sv) -> Update` ;
  - `apply_remote(doc, update) -> Projected` : écrit le blob, attribue un `seq`, projette et renvoie les uuid modifiés ;
  - `peer` / `set_peer` (curseur, dernière adresse), `devices()`, `adopt_space(space_id, remote_space_doc)` (règle 8 du §4).
- **`SyncProfiles`** :
  - `profile_for(space_id) -> Option<ProfileId>`, `with_store(profile_id, f)` ;
  - `create_profile(name) -> ProfileId`, `synced_profiles()`.

## 6. Appairage

**Sur A, « Lier un appareil »**
1. Liste des profils à proposer, avec le profil actif coché par défaut. Un espace est créé pour chaque profil coché qui n'en a pas.
2. A affiche un **QR code** et, en dessous, un **code à 6 chiffres**. Le mode reste ouvert 5 minutes, avec un compte à rebours. L'annonce mDNS ajoute `pairing=1` et le nom de l'appareil, **jamais le code**.
3. Après 10 échecs de vérification, A génère un nouveau code et un nouveau QR.

**Contenu du QR code** : `daftr-pair:1:<adresses IP:port>:<empreinte de la clé publique de A>:<code>`. Ce format est stable dès maintenant, pour le futur client mobile (adresses séparées par des virgules, IPv6 entre crochets : figé au lot 3).

**Sur B, « Se lier à un appareil »**
- **Appareil avec appareil photo** : il scanne le QR code, se connecte à l'adresse, vérifie que la clé de A correspond à l'empreinte, puis lance la vérification avec le code.
- **Appareil sans appareil photo** (le desktop en v1) : on saisit le code. B ouvre une connexion `Pair` vers chaque appareil en mode appairage découvert sur le LAN (ou vers l'`IP:port` saisi), au plus 4 en parallèle. Seul le bon appareil valide le code.
- Plusieurs utilisateurs peuvent s'appairer en même temps : chaque A a son propre code, et un mauvais A échoue sans rien apprendre.

**Vérification** (connexion `Pair`)
1. Handshake Noise XX.
2. SPAKE2 avec le code comme mot de passe.
3. Confirmation de clé par un MAC sur le hash du handshake Noise : le code est lié à cette connexion précise.

**Après la vérification**
1. A et B s'ajoutent mutuellement dans `devices.json` (origine `paired`).
2. A envoie l'**offre** : pour chaque profil coché, `{space_id, nom, nombre de notes}`.
3. B choisit, profil par profil :
   - **Nouveau profil** (créé via l'API des profils, avec le nom proposé) ;
   - **Fusionner avec <profil>** (seulement les profils non synchronisés, §4.8) ;
   - **Ignorer**.
4. A ajoute B dans `space.devices` de chaque profil accepté.
5. **La synchronisation démarre immédiatement** sur la même connexion (passage en session `Sync`), avec un rattrapage complet. B affiche la progression (« Synchronisation initiale : 120 / 450 notes »).

**Ajouter un profil plus tard avec un appareil déjà lié** (sans code)
- Dans les réglages du profil, « Synchroniser avec… », puis choix d'un appareil de confiance.
- L'offre est envoyée immédiatement si l'appareil est présent. Sinon elle est mise en attente et envoyée dès son apparition sur le réseau.
- L'appareil distant affiche une notification « <A> propose de synchroniser <profil> » avec les mêmes choix (nouveau, fusionner, ignorer). **La synchronisation démarre immédiatement** après l'acceptation.

## 7. Synchronisation

**Écoute et adresses**
- **Un seul moteur par installation**, une écoute TCP sur le port configuré, toutes interfaces.
- **Toute connexion dont l'IP source n'est pas privée est refusée avant le handshake** : RFC 1918, `169.254/16`, `fc00::/7`, `fe80::/10`, loopback pour les tests. Le moteur **ne compose jamais** une adresse non privée, même saisie à la main.
- Annonce mDNS dès qu'au moins un profil est synchronisé et que la synchronisation n'est pas en pause. Champ TXT : empreinte de la clé publique et version du protocole. **Jamais de `space_id`.**
- Messages `postcard` dans des trames Noise. Taille maximale d'un message : 16 Mio, au-delà on rejette. Le premier message indique le type de session (`Pair` ou `Sync`).

**Session `Sync`** : **une seule connexion par paire d'appareils**, qui transporte tous leurs profils communs.
1. Handshake Noise XX. La clé distante doit être connue et non révoquée.
2. `Hello {protocol_version, device_name, spaces}`. Chaque côté n'annonce que les espaces où l'autre figure dans `S.devices`. Les espaces synchronisés sont l'intersection, moins les profils dont la synchronisation est désactivée.
3. **Rattrapage**, par espace et dans chaque sens :
   - `Changed [(doc_id, sv)]` pour les documents dont `seq > acked_seq` du pair, `space` en premier ;
   - le pair répond `Diff {doc_id, update, sv}` et l'émetteur renvoie son propre diff ;
   - à la fin, `Ack {seq}` met à jour `acked_seq` ;
   - les documents sont appliqués par lots de 200 par transaction, et les événements vers l'interface sont regroupés ;
   - premier contact : `acked_seq = 0`, donc tout est échangé.
4. **Temps réel** : la connexion reste ouverte. Chaque modification locale de n'importe quel profil synchronisé (après le debounce de sauvegarde actuel, plus 1 s de regroupement) est poussée en `Update {space_id, doc_id, update}`. Un renvoi à l'appareil d'origine ne coûte rien, car le diff est vide.
5. **Offres** de profil (§6) : message `Offer` / `OfferReply` dans la même session.

**Déclencheurs** (automatiques, sans polling)
- fin d'un appairage ou acceptation d'une offre → rattrapage immédiat ;
- apparition mDNS d'un appareil de confiance → connexion (en cas de collision, l'appareil à la plus petite clé initie) ;
- changement de réseau → nouvelle tentative vers `last_addr` ;
- modification locale → poussée vers les appareils connectés ;
- « Synchroniser maintenant » → nouvelle tentative immédiate vers tous les appareils de confiance ;
- échec → backoff de 30 s à 10 min, seulement vers `last_addr`.

**Pause globale** : ferme les connexions, arrête l'écoute et l'annonce. Les modifications restent en local et partent à la reprise.

**Retraits**
- **« Arrêter de synchroniser ce profil avec <X> »** : `revoked_at` dans `S.devices`. X ne peut plus échanger ce profil. Ses données restent chez lui.
- **« Retirer l'appareil <X> »** : révocation dans `devices.json` et dans tous les espaces communs. C'est définitif : un appareil révoqué ne peut pas se réajouter sans un nouvel appairage. Les données déjà copiées restent chez lui, et l'interface doit le dire.

## 8. Intégration de l'éditeur (v1, sans Yjs côté front)

- Le backend émet `notes-synced {profile_id, uuids}` après chaque `apply_remote`. Pour le profil actif, les listes, la sidebar, le graphe et les backlinks se rafraîchissent. Les autres profils n'ont rien à rafraîchir.
- **La note ouverte reçoit une mise à jour** :
  1. vider la sauvegarde en attente ; le backend la fusionne dans le CRDT ;
  2. relire le contenu ;
  3. l'appliquer dans CodeMirror sous forme de **transaction minimale** (diff), pour que la sélection et les curseurs soient mappés et non réinitialisés ;
  4. afficher un indicateur discret « modifié depuis <appareil> ».
- **Note ouverte supprimée à distance** : l'onglet passe en lecture seule, avec un bandeau « supprimée sur <appareil> » et une action « Restaurer ».

## 9. Interface

**Réglages → Synchronisation** (appareil)
- Nom de cet appareil, port d'écoute, pause globale, « Synchroniser maintenant ».
- « Lier un appareil » (choix des profils, puis QR code et code avec compte à rebours) et « Se lier à un appareil » (saisie du code, ou `IP:port` en repli).
- Écran de choix de destination par profil proposé (nouveau, fusionner, ignorer), puis progression de la synchronisation initiale.
- Appareils de confiance : nom, présent ou absent du réseau, profils synchronisés avec lui, dernière synchronisation, « Retirer ».

**Réglages du profil**
- Activer ou désactiver la synchronisation de ce profil.
- « Synchronisé avec : … », « Synchroniser avec… » (offre à un appareil de confiance), « Arrêter avec <X> ».

**Barre d'état** : désactivée / en pause / à jour (N appareils) / synchronisation… / aucun appareil sur ce réseau / erreur (tooltip avec la cause, par exemple « réseau public bloqué par le pare-feu »).

**Notifications** : appareil lié, offre de profil reçue, synchronisation initiale terminée.

Toutes les chaînes sont traduites via `src-tauri/crates/domain/src/i18n.rs` et le système i18n du front.

## 10. Budgets, plateformes, sécurité

**Budgets.** Objectifs initiaux : moins de 1 Mo de binaire, moins de 5 Mo de RAM au repos, CPU ≈ 0. **Décision du 2026-11-19** (après le spike, voir [mesures-spike.md](mesures-spike.md)) : le surcoût binaire mesuré, ~1,7 Mio, est accepté ; la pile est conservée telle quelle (Noise BLAKE2s inchangé) ; la détection d'IP périodique de `mdns-sd` est coupée (`set_ip_check_interval(0)`) et l'annonce est refaite sur changement de réseau.

**Plateformes**
- **Windows** : la première écoute déclenche l'invite du pare-feu. L'installateur NSIS ajoute une règle entrante TCP sur le port, **profil Privé uniquement** (plus l'UDP 5353 entrant). Un réseau marqué « Public » bloque : le signaler dans l'indicateur.
- **macOS** : déclarer `NSLocalNetworkUsageDescription` et `NSBonjourServices` (`_daftr-sync._tcp`) dans `Info.plist`. Sans ces clés, macOS bloque silencieusement le LAN.
- **Linux** : documenter l'ouverture du port TCP et du port UDP 5353 si un pare-feu (ufw, firewalld) est actif.

**Sécurité**
- Transport chiffré et authentifié des deux côtés (Noise XX, clés statiques).
- Appairage protégé par PAKE et lié au handshake ; code renouvelé après 10 échecs ; aucun secret dans l'annonce mDNS.
- Les `space_id` ne sont révélés qu'aux appareils authentifiés et membres de l'espace concerné.
- Données au repos : inchangées (SQLite en clair, comme aujourd'hui).
- Le contenu reçu est une donnée non fiable : on valide les tailles, les uuid et les références, et on ne fait jamais `unwrap` sur une entrée réseau.

## 11. Lots

Tous les lots sont livrés dans la même branche, `NF/feat/sync-lan-261123`, et une seule PR vers `recipe`.

| Lot | Contenu | Dépend de |
|---|---|---|
| 0 — spike | Binaire jetable : `mdns-sd` + `snow` + `spake2` + `yrs`, deux processus qui s'appairent par code et s'échangent un document en LAN. Mesures du §10 | — |
| 1 — modèle | Migration §3, soft delete + filtres de lecture + FTS, uuid, liens `note://uuid` (backend, front, MCP), blob tenu à jour par l'adaptateur, `adopt_space` | Profils |
| 2 — CRDT pur | `app::sync` §4 + ports `SyncStore` / `SyncProfiles` / `SyncEvents` | — |
| 3 — protocole | Crate `adapter-sync` : écoute, filtre d'adresses privées, Noise, PAKE, mDNS, appairage (QR et code), offres, session multi-espaces, rattrapage, temps réel, déclencheurs, backoff, pause, révocations | 2 |
| 4 — intégration | `desktop` : moteur unique, `impl SyncProfiles` (base du profil actif partagée, autres ouvertes à la demande), fichiers de `sync/`, commandes, événements, règle de pare-feu NSIS, clés `Info.plist` | 1, 3 |
| 5 — front | Réglages de l'appareil et du profil, indicateur, notifications, écrans d'appairage (QR + code, saisie, destination, progression), application des mises à jour dans l'éditeur (§8) | 4 |
| 6 — doc | Réécrire [plan.md](plan.md) selon ce document, ajouter une section README (fonctionnement, appairage, pare-feu) | 5 |

Phases ultérieures, hors de ce lot : client mobile (scan du QR code), scan du QR par webcam sur desktop, export et import d'un paquet de modifications par fichier (clé USB) pour les appareils jamais présents en même temps sur le réseau, historique des versions, `y-codemirror.next`.

## 12. Tests obligatoires

**Données**
- **Migration** sur une base fixture de la version actuelle : uuid uniques, liens réécrits, FTS cohérente, seconde exécution sans effet, `daftr.db.pre-sync` créé.
- **Convergence** (propriété) : N appareils, éditions concurrentes aléatoires (accents, emojis, CRLF), échanges dans un ordre aléatoire et avec des doublons → projections identiques.
- **Contenu** : `project(apply_changes(doc, x)).content == x` sur des contenus aléatoires (indices UTF-16).
- **Suppression** : suppression concurrente d'une modification → note restaurée ; suppression qui a vu la modification → note supprimée, de manière identique sur tous les appareils.
- **Dossiers** : cycle provoqué et réparé ; note dans un dossier inconnu, puis le dossier arrive et la note s'y range.
- **Fusion d'un profil existant** : dossiers de même chemin unifiés, Inbox unique, notes identiques dédoublonnées, autres conservées, sauvegarde créée ; fusion refusée pour un profil déjà synchronisé.

**Appairage**
- Code correct → lié. Mauvais code → refusé. Deux appairages simultanés sur le même LAN avec des codes différents → chacun aboutit au bon appareil.
- 10 échecs → nouveau code. Mode expiré → plus annoncé, connexions `Pair` refusées.
- Scan du QR code avec une empreinte de clé différente → refusé.
- Synchronisation complète terminée juste après l'acceptation, sans autre action.

**Profils et sessions**
- A (P1, P2), B (Q1), C (P1, P2, Q1) : A↔B n'échangent rien ; A↔C synchronisent P1 et P2 sur une seule connexion ; B↔C synchronisent Q1.
- Profil inactif et profil verrouillé synchronisés ; profil désactivé ignoré.
- Propagation transitive : A↔B puis B↔C sur le même espace → C a les modifications de A, sans appairage A↔C.
- Offre à un appareil absent → envoyée à son apparition, synchronisation immédiate après acceptation.

**Sécurité**
- Connexion depuis une IP non privée refusée avant le handshake ; composition d'une IP non privée refusée.
- Clé inconnue ou révoquée refusée ; espace non partagé jamais révélé dans `Hello` ; message trop gros refusé.
- Pause → plus d'écoute ni d'annonce ; reprise → rattrapage.

## 13. Critères d'acceptation

1. Deux PC sur le même réseau local : A affiche un QR code et un code, B saisit le code. Les deux sont liés et **la synchronisation complète démarre aussitôt**.
2. Ensuite, une note modifiée sur l'un apparaît sur l'autre en moins de 3 s, **quel que soit le profil actif** des deux côtés.
3. Modifications hors ligne sur les deux PC → au retour sur le même réseau, la fusion se fait automatiquement, sans perte ni dialogue.
4. Un troisième appareil lié au seul premier reçoit tout, y compris les modifications du second.
5. Deux utilisateurs qui s'appairent en même temps sur le même LAN ne se mélangent pas.
6. L'appairage fonctionne aussi via `IP:port` quand le mDNS est filtré.
7. Aucune connexion de synchronisation hors réseau local, vérifiée par test et par une capture rapide pendant la recette.
8. Sans profil synchronisé, l'app se comporte exactement comme en v1.7.0, consommation comprise.
9. Budgets du §10 respectés et mesurés dans la PR.
