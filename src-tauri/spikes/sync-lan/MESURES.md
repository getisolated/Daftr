# Lot 0 — spike de la synchronisation LAN : mesures

> Binaire **jetable** (ne pas livrer, ne pas merger) qui valide la pile technique de la spec
> « synchronisation LAN » (§1) et mesure les budgets de son §10.
> Mesures du 2026-11-19 : Windows 11 Enterprise, AMD Ryzen 9 8945HS (8 cœurs / 16 threads),
> rustc 1.96.0, Wi-Fi en profil « Privé », adaptateurs Hyper-V/WSL présents.

## Verdict : **go sous conditions**

| Budget §10 | Objectif | Mesuré | Statut |
|---|---|---|---|
| Binaire (surcoût dans l'app réelle) | < 1 Mo | **+1,69 Mio** (+1 771 520 o, +7,8 %) | **DÉPASSÉ** |
| RAM au repos (octets privés, écoute + mDNS) | < 5 Mo | **+0,7 à +0,9 Mio** vs référence | OK |
| RAM au repos (working set, indicatif) | < 5 Mo | +2,4 à +4,4 Mio vs référence | OK, mais bruité |
| CPU au repos (écoute + mDNS, réglages par défaut) | ≈ 0 | **≈ 47 ms / min** (0,08 % d'un cœur) | ≈ 0, pas nul |
| CPU au repos, `mdns-sd` sans détection d'IP (`--ip-check 0`) | ≈ 0 | **0 ms sur 5 min** | OK |

La pile fonctionne de bout en bout (appairage, refus, convergence, QR). **Le budget binaire
est dépassé de ~70 %**, à cause de `mdns-sd` (~46 % du surcoût) et de `yrs` (~29 %, imposé
par le format Yjs). Conditions proposées pour continuer : remplacer ou alléger la découverte
mDNS et repasser sous 1 Mo, **ou** faire valider un budget révisé (~1,7 Mio) ; désactiver la
détection périodique d'IP de `mdns-sd` (voir §5).

## 1. Ce que le spike démontre

Projet Cargo indépendant (`[workspace]` vide, `target/` ignoré) :

| Module | Rôle |
|---|---|
| [src/addr_filter.rs](src/addr_filter.rs) | Filtre d'adresses locales (RFC 1918, 169.254/16, fc00::/7, fe80::/10, loopback, IPv4 mappées en IPv6) |
| [src/transport.rs](src/transport.rs) | Trames `u16` + handshake Noise `XX_25519_ChaChaPoly_BLAKE2s` ; messages `postcard` préfixés `u32`, découpés en trames Noise ; 16 Mio max vérifiés **avant** allocation |
| [src/pairing.rs](src/pairing.rs) | SPAKE2 (`spake2`, Ed25519) dans le canal Noise, identités = clés statiques Noise, confirmation HMAC-SHA256 sur le hash du handshake ; B confirme en premier |
| [src/crdt.rs](src/crdt.rs) | Note `yrs` (`OffsetKind::Utf16`, client id 53 bits dérivé de la clé), contenu complet → opérations `Y.Text` via `similar` |
| [src/session.rs](src/session.rs) | State vector → diff dans les deux sens, vérification de convergence |
| [src/discovery.rs](src/discovery.rs) | Annonce et recherche mDNS `_daftr-sync._tcp.local.` (TXT : `fp`, `v`, `pairing=1` ; jamais le code) |
| [src/qr.rs](src/qr.rs) | `daftr-pair:1:<ip:port>:<empreinte>:<code>` et rendu SVG (`qrcode` sans `image`) |
| [src/listen.rs](src/listen.rs), [src/pair.rs](src/pair.rs) | CLI `listen` (A) et `pair <code>` (B) |
| [src/bin/reference.rs](src/bin/reference.rs), `src/bin/size-*.rs` | Binaires de mesure (référence, surcoût par crate) |
| [scripts/mesure-repos.ps1](scripts/mesure-repos.ps1), [scripts/app_sync_probe.rs](scripts/app_sync_probe.rs) | Mesure RAM/CPU au repos ; sonde pour mesurer le surcoût dans l'app réelle |

### Validation

```text
$ cargo build --release      → Finished `release` profile [optimized]
$ cargo clippy --all-targets -- -D warnings → aucun avertissement
$ cargo fmt --check          → ok
$ cargo test                 → test result: ok. 17 passed; 0 failed
```

Tests : filtre d'adresses (acceptées / refusées), handshake (clés authentifiées, même hash),
message de 3 trames Noise reconstruit, message > 16 Mio refusé à l'envoi **et** à la réception
(longueur forgée), bon code → lié, mauvais code → refusé des deux côtés, MAC lié au hash du
handshake, convergence (accents, emojis, CRLF), mises à jour en double et désordonnées,
aller-retour contenu ↔ `Y.Text` en UTF-16, URI du QR avec IPv4 et IPv6.

### Démonstration (deux processus, même machine)

```text
$ sync-lan-spike listen --qr sync-spike-qr.svg
[A] écoute sur 0.0.0.0:47820 et [::] : true (IP LAN 10.0.2.64)
[A] empreinte 18f17cd1e838ced00db24d5735e1d683
[A] code d'appairage 226562
[A] QR code "daftr-pair:1:10.0.2.64:47820:18f17cd1e838ced00db24d5735e1d683:226562" écrit dans …\sync-spike-qr.svg
[A] annonce mDNS _daftr-sync._tcp.local. active

### B : mauvais code, découverte mDNS
[B] mDNS : appareil 18f17cd1e838ced00db24d5735e1d683 sur [10.0.2.64:47820, 127.0.0.1:47820,
    172.21.160.1:47820, 172.29.64.1:47820, [fe80::92fa:…%15]:47820, … 14 adresses]
[B] connecté à 10.0.2.64:47820
[B] Noise XX ok en 2.9615ms, pair 18f17cd1e838ced00db24d5735e1d683
[B] appairage refusé : code refusé par l'appareil distant          (exit=1)

### B : bon code, découverte mDNS
[B] connecté à 10.0.2.64:47820
[B] Noise XX ok en 2.2487ms, pair 18f17cd1e838ced00db24d5735e1d683
[B] code accepté, appareils liés en 3.3375ms
[B] rattrapage initial : "Liste de courses :\n- pain\n- lait\n"
[B] convergence ok, contenu "Liste de courses :\r\n- pain complet\n- lait\n- œufs 🥚\n"
[B] terminé en 4.6757ms                                             (exit=0)

### B : repli IP:port en loopback, puis IP LAN + empreinte du QR → mêmes sorties, exit=0
### B : mauvaise empreinte
Error: empreinte inattendue : 18f17cd1… au lieu de 00000000000000000000000000000000
### B : IP publique saisie
[B] 8.8.8.8:47820 ignorée : adresse non locale                     (aucun paquet émis)

### Journal de A
[A] 10.0.2.64:50164 : mauvais code, refusé (1/10)
[A] 10.0.2.64:50165 : code correct, appareils liés en 1.9933ms
[A] 10.0.2.64:50165 : convergence ok en 3.2734ms, contenu "Liste de courses :\r\n- pain complet\n- lait\n- œufs 🥚\n"
[A] 127.0.0.1:50166 : convergence ok en 3.0445ms, contenu "…"
… 9 mauvais codes supplémentaires …
[A] 127.0.0.1:50179 : mauvais code, refusé (10/10)
[A] trop d'échecs : nouveau code 934521
```

A édite « pain » → « pain complet », B passe la 1re ligne en CRLF et ajoute « œufs 🥚 » sans
voir l'édition de A : les deux contenus convergent. Le refus d'une IP source non locale côté A
est couvert par les tests (impossible à provoquer sur une seule machine).

## 2. Binaire

### Méthode

Profil `release` aligné sur `src-tauri/Cargo.toml` : `codegen-units = 1`, `lto = true`,
`strip = true` (opt-level 3 et `panic = unwind` par défaut, comme l'app).

1. **Surcoût réel dans l'app** (chiffre de référence) : copie de `src-tauri/`, `package.json`,
   `assets/` et du `frontend/dist` existant hors du dépôt, `cargo build --release -p desktop
   --locked`, puis même build après avoir lié le spike via [scripts/app_sync_probe.rs](scripts/app_sync_probe.rs)
   (toute la pile appelée, derrière une variable d'environnement pour que LTO ne l'élimine pas).
   Les crates déjà présentes dans l'app (tokio, serde, serde_json, sha2, rand, socket2,
   thiserror, ring…) sont donc mutualisées.
2. **Détail par crate** (approché) : `reference.exe` = runtime tokio multi-thread + écoute TCP
   + anyhow (ce que l'app embarque déjà) ; chaque `size-<crate>.exe` = référence + usage
   réaliste de la seule crate. Surcoût = différence de taille. `cargo bloat` n'a pas été
   utilisé (binaires PE strippés).

### Résultats

| Binaire | Taille (o) | Surcoût |
|---|---:|---:|
| `desktop.exe` (app v1.7.0, release) | 22 852 608 | — |
| `desktop.exe` + pile du spike | 24 624 128 | **+1 771 520 o = 1 730 Kio (1,69 Mio)** |
| `reference.exe` | 438 784 | — |
| `sync-lan-spike.exe` (spike complet, autonome) | 2 376 192 | +1 892 Kio (1,85 Mio) |

Détail par crate, autonome (somme 1 790 Kio ≈ spike complet ; dans l'app le total réel est
~9 % plus bas grâce aux dépendances partagées) :

| Crate (avec ses dépendances nouvelles) | Surcoût | Part |
|---|---:|---:|
| `mdns-sd` 0.22 (sans features : ni `async`, ni `log`) | **828,5 Kio** | 46 % |
| `yrs` 0.28 | **526,5 Kio** | 29 % |
| `similar` 3.2 (`diff_chars`) | 151,0 Kio | 8 % |
| `snow` 0.10 (resolver par défaut sans `std`, sans AES-GCM) | 147,5 Kio | 8 % |
| `spake2` 0.4 + `hmac` 0.12 | 79,0 Kio | 4 % |
| `qrcode` 0.15 (`std` + `svg`, sans `image`) | 47,5 Kio | 3 % |
| `postcard` 1.1 (`alloc`) | 10,0 Kio | 0,5 % |

Réglages de features déjà appliqués : `snow` sans `std` (sa feature `std` active `ring/std`
donc tire `ring`) et sans AES-GCM ; `mdns-sd` sans `async`/`logging` ; `qrcode` sans `image` ;
`postcard` sans `heapless-cas`.

## 3. RAM et CPU au repos

### Méthode

[scripts/mesure-repos.ps1](scripts/mesure-repos.ps1) lance en parallèle `reference.exe`, le
spike `listen --no-mdns`, le spike `listen` (mDNS) et le spike `listen --ip-check 0`, attend
N s, relève `WorkingSet64`, `PrivateMemorySize64`, le nombre de threads et
`TotalProcessorTime` (`Get-Process`), attend encore N s et calcule le CPU consommé sur la
seconde fenêtre. Aucune connexion pendant la mesure. Résolution du temps CPU Windows :
15,6 ms (les valeurs sont des multiples de ce pas).

### Résultats

Fenêtre de 60 s (deux passes) :

| Processus | Octets privés | Working set | Threads | CPU sur 60 s |
|---|---:|---:|---:|---:|
| référence | 1 556–1 588 Kio | 5 220–5 848 Kio | 17 | 0 ms |
| spike sans mDNS | 1 796 Kio | 6 176–8 040 Kio | 17 | 0 ms |
| spike + mDNS | 2 288–2 452 Kio | 8 256–8 280 Kio | 20 | 31–47 ms |
| spike + mDNS, `--ip-check 0` | 2 320 Kio | 8 232 Kio | 20 | 16 ms |

Fenêtre de 300 s (passe longue, pendant un build en parallèle : working set plus bruité) :

| Processus | Octets privés | Working set (fin) | Threads | CPU sur 300 s |
|---|---:|---:|---:|---:|
| référence | 1 620 Kio | 5 484 Kio | 19 | 0 ms |
| spike sans mDNS | 1 856 Kio | 8 128 Kio | 19 | 0 ms |
| spike + mDNS | 2 528 Kio | 9 920 Kio | 22 | **234 ms** (≈ 47 ms/min) |
| spike + mDNS, `--ip-check 0` | 2 440 Kio | 9 768 Kio | 22 | **0 ms** |

- **RAM** : surcoût privé de l'écoute + mDNS ≈ **+0,7 à +0,9 Mio** (objectif < 5 Mo tenu).
  Le working set inclut les pages de code du binaire plus gros et varie avec la pression
  mémoire : +2,4 à +4,4 Mio, toujours sous 5 Mo.
- **Threads** : `mdns-sd` ajoute **+3 threads** : son unique thread démon (seul
  `thread::Builder` de la crate) et, probablement, deux threads du pool Windows créés par
  les sockets multicast / appels IP Helper (non vérifié). Le runtime tokio multi-thread de
  référence en a déjà 17–19.
- **CPU** : sans mDNS, 0. Avec `mdns-sd` par défaut, ≈ 47 ms/min, dû à la **détection des
  changements d'IP toutes les 5 s** (énumération des interfaces). Avec `set_ip_check_interval(0)`,
  0 ms sur 5 min. Démarrage du démon : 90 à 390 ms CPU (une fois).

## 4. Dépendances ajoutées

`cargo tree` du spike : 85 paquets (cible Windows, dépendances normales). Lié à l'app, le
`Cargo.lock` gagne **34 crates** :

```text
aead arc-swap blake2 bstr chacha20@0.9.1 chacha20poly1305 cipher cobs curve25519-dalek
curve25519-dalek-derive dashmap embedded-io@0.4 embedded-io@0.6 fiat-crypto flume
hashbrown@0.14.5 hkdf hmac if-addrs inout mdns-sd opaque-debug poly1305 postcard qrcode
rand_core@0.6.4 similar smallstr snow socket-pktinfo spake2 spin universal-hash yrs
```

Doublons de version **créés** (`cargo tree -d -p desktop`, 32 → 35 noms en doublon) :
`chacha20` 0.9 (snow) / 0.10 (rand de l'app), `rand_core` 0.6 (snow, spake2) / 0.10 (app).
`hashbrown` 0.14 (yrs) s'ajoute aux versions déjà multiples de l'app. Déjà présents et
réutilisés : tokio, serde, serde_json, sha2 0.10, digest 0.10, subtle, zeroize, socket2,
getrandom 0.2/0.3, fastrand, async-lock, thiserror 2, smallvec, windows-sys.

Arbre par crate nouvelle : `mdns-sd` → flume, if-addrs, socket-pktinfo, spin (+ socket2, mio
déjà là) ; `snow` → blake2, chacha20poly1305 (chacha20 0.9, poly1305, cipher…),
curve25519-dalek ; `spake2` → curve25519-dalek (partagé avec snow), hkdf, hmac, sha2 ;
`yrs` → arc-swap, dashmap, smallstr, hashbrown 0.14 (+ serde_json, async-lock déjà là) ;
`postcard` → cobs ; `similar`, `qrcode` : aucune.

## 5. Pistes pour repasser sous 1 Mo (non implémentées)

1. **Découverte mDNS (≈ 830 Kio)** — le levier principal :
   - mDNS maison minimal sur `socket2` (déjà dans l'app) : annonce + requête PTR/SRV/TXT/A/AAAA
     d'un seul service, quelques centaines de lignes, estimé < 100 Kio ;
   - ou évaluer `libmdns` (répondeur seul, il manque la recherche) / `simple-mdns` ;
   - ou API natives (Windows `DnsServiceRegister`/`DnsServiceBrowse`, macOS `dns_sd`) ; Linux
     resterait à couvrir sans Avahi, contraire au choix « Rust pur » de la spec.
2. **`yrs` (≈ 530 Kio)** : pas d'alternative compatible Yjs ; à accepter.
3. **`similar` (≈ 150 Kio)** : remplaçable par un diff maison (préfixe/suffixe communs puis
   Myers caractère sur la zone modifiée, quelques dizaines de lignes). Gain estimé ~130 Kio.
4. **`snow` (≈ 150 Kio)** : `ring` est **déjà embarqué** par l'app (rustls de
   `tauri-plugin-updater`). Le resolver `ring` de snow couvre X25519 et ChaChaPoly mais pas
   BLAKE2 : passer à `Noise_XX_25519_ChaChaPoly_SHA256` réutiliserait `ring`, supprimerait
   les doublons `chacha20`/`rand_core` et économiserait l'essentiel de snow. À mesurer.
5. `spake2` + `hmac` (≈ 80 Kio) : `hmac` peut venir de `ring::hmac` ; `curve25519-dalek` reste
   nécessaire pour SPAKE2.

Avec 1 et 3 (voire 4), le surcoût estimé tombe vers 0,8–0,9 Mio.

## 6. Frictions rencontrées (pour le lot 3)

**Noise / PAKE**
- `snow` : le hash du handshake n'est lisible que sur `HandshakeState` ; il faut le copier
  **avant** `into_transport_mode()`. Trames Noise limitées à 65 535 o : un message de 16 Mio
  doit être découpé (préfixe `u32` puis trames de 65 519 o de clair).
- `snow` : la feature `std` active `ring/std`, donc tire `ring` ; `default-features = false`
  avec les seuls `use-chacha20poly1305`, `use-blake2`, `use-curve25519`, `use-getrandom`.
- `spake2` 0.4 n'a pas de paramètre de contexte : la liaison au canal se fait par les
  identités (clés statiques Noise) et par la confirmation de clé HMAC sur le hash du
  handshake, en dehors de la crate. Ordre retenu : B (qui compose) confirme en premier, A ne
  renvoie sa confirmation qu'après vérification. SPAKE2 garantit un seul essai par session
  dans les deux sens (un faux appareil « en mode appairage » ne gagne qu'un essai en ligne).
- `spake2` 0.4 : erreurs sans `std::error::Error` sauf feature `std` ; dépend de
  `rand_core` 0.6 (doublon). Sa variante Ed25519 suit `python-spake2`/magic-wormhole, pas
  la RFC 9382 : **le futur client mobile devra utiliser exactement la même construction**
  (à vérifier avant de figer le protocole). Crate peu active (dernière version 2023).
- `qrcode` sans features par défaut : `QrError` n'implémente `std::error::Error` qu'avec
  la feature `std` (à garder).

**mDNS (`mdns-sd`)**
- L'annonce `enable_addr_auto()` publie **toutes** les interfaces : 14 adresses ici, dont
  127.0.0.1, ::1, les vEthernet Hyper-V/WSL (172.21.160.1, 172.29.64.1) et chaque IPv6
  lien-local sur plusieurs index d'interface. Un pair distant essaierait des adresses
  injoignables (3 s chacune en série) et **127.0.0.1 le ferait se connecter à lui-même**
  (stoppé seulement par l'empreinte). Lot 3 : exclure loopback et interfaces virtuelles de
  l'annonce (`IfKind`), écarter le loopback côté recherche hors tests, essayer les adresses
  en parallèle (style happy eyeballs).
- Les adresses arrivent en **plusieurs `ServiceResolved`** (la première résolution n'avait
  que des IPv6 lien-local) : il faut fusionner pendant un court délai (700 ms ici).
- IPv6 lien-local : il faut le `scope_id` (`%15`) pour se connecter ; et Windows ouvre
  `[::]` en IPV6_V6ONLY, d'où une **seconde écoute** IPv6 en plus de `0.0.0.0`.
- Détection d'IP toutes les 5 s par défaut = seul coût CPU au repos ; la désactiver et
  déclencher la ré-annonce sur événement de changement de réseau.
- Sur une même machine, la découverte fonctionne (boucle multicast) ; elle ne prouve rien
  sur le filtrage mDNS d'un vrai réseau (isolation Wi-Fi) : à tester en recette sur deux PC.

**Windows**
- La première écoute de chaque exécutable déclenche l'invite du pare-feu, qui crée des
  règles **par chemin d'exécutable**, TCP **et** UDP (UDP = mDNS 5353), ici sur le profil
  « Public » alors que le Wi-Fi est « Privé ». La règle NSIS du lot 4 doit cibler le port et
  le profil Privé, et prévoir l'UDP 5353 entrant.
- Le trafic vers sa propre IP LAN ne traverse pas le pare-feu comme un vrai pair : la
  recette doit se faire sur deux machines.

**Format et CRDT**
- Le format QR `daftr-pair:1:<adresses IP:port>:<empreinte>:<code>` est ambigu avec IPv6
  (`:` partout) : analyse depuis les deux extrémités, adresses séparées par `,` et IPv6 entre
  crochets. À figer explicitement dans la spec.
- `yrs` 0.28 : `ClientID` 53 bits (`ClientID::new`, compatible Yjs ≥ 14) ; l'ancienne
  feature `small-client` (32 bits) casse la compatibilité avec un Yjs récent.
- `similar::diff_chars` donne des indices en caractères Unicode ; il faut les convertir en
  unités UTF-16 pour `Y.Text` (`char::len_utf16`). Un `timeout` (200 ms) borne le coût sur
  les très gros contenus (diff alors moins minimal, mais correct).

## 7. Reproduire

```powershell
cd src-tauri/spikes/sync-lan
cargo build --release ; cargo test ; cargo clippy --all-targets -- -D warnings
# Démo
target/release/sync-lan-spike.exe listen --qr sync-spike-qr.svg      # terminal 1
target/release/sync-lan-spike.exe pair 000000                         # refus
target/release/sync-lan-spike.exe pair <code>                         # via mDNS
target/release/sync-lan-spike.exe pair <code> --addr 127.0.0.1:47820  # repli IP:port
# Tailles
Get-ChildItem target/release/*.exe | Select-Object Name, Length
# RAM / CPU (voir l'en-tête du script)
powershell -File scripts/mesure-repos.ps1 -Dir <dossier> -Seconds 60
```

Versions résolues : `mdns-sd` 0.22.0, `snow` 0.10.0, `spake2` 0.4.0, `hmac` 0.12.1,
`yrs` 0.28.1, `similar` 3.2.1, `postcard` 1.1.3, `qrcode` 0.15.0, `sha2` 0.10.9,
`rand` 0.10.4, `tokio` 1.54.1 (1.53.3 dans l'app), `serde` 1.0.229.
