# Migration Electron → Rust + Tauri 2

Branche `NF/refactor/tauri-main-process-261019`, partie de `main` (475f7d2).
Renderer Angular conservé ; le main process (`electron/` (supprimé), ~3000 lignes) est réécrit en Rust.

## Décisions prises

| Sujet                 | Choix                                                                                                          |
| --------------------- | -------------------------------------------------------------------------------------------------------------- |
| Arborescence          | Angular déplacé dans `frontend/` (`git mv`), Rust dans `src-tauri/`                                            |
| Données               | Même dossier qu'Electron : `%APPDATA%/everything-note/` (champ `name` du `package.json`) (`evnote.db`, `settings.json`), schéma SQLite inchangé |
| Mise à jour           | `tauri-plugin-updater` (signature minisign, `latest.json` au format Tauri)                                     |
| Bindings TS           | `ts-rs` (stable, simple) plutôt que `specta` (tauri-specta encore en RC)                                       |
| `domain`              | std seul ; DTO miroirs (serde + ts-rs) dans `presentation`, conversions `From`                                 |
| Overlay sans acrylic  | Panneau translucide uni, plus de capture d'écran                                                               |
| Utilisateurs Electron | `latest.json` hybride temporaire + hook NSIS de désinstallation ; pas de migration du localStorage             |
| Commits               | Un commit par étape, sans push                                                                                 |

## Organisation des crates (`src-tauri/crates/`)

```
platform ◄── app ◄── adapter-sqlite / adapter-fs / presentation ◄── desktop
domain   ◄───┘
```

| Crate            | Contenu (origine Electron)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `platform`       | Init `tracing`, debounce générique, helpers de chemins canoniques (8.3 → long)                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `domain`         | Pur, std seul : règles settings (`settings-schema.ts`), géométrie overlay (`overlay-geometry.ts`), versions (`update-manifest.ts`), noms/couleurs/FTS/LIKE/tags (`database.ts`), liens + graphe + recherche de collections (`collections-scan.ts`), noms d'export + ZIP paths (`export.ts`), textes i18n (`i18n.ts`) |
| `app`            | Cas d'usage (notes, dossiers, collections, settings, export), ports (traits : dépôts, système de fichiers, dialogues, corbeille, publication d'événements), service de collections (watch/scan/search sur thread dédié)                                                                                                                                                                                                                                                                                                                          |
| `adapter-sqlite` | `rusqlite` : schéma, migrations idempotentes, triggers FTS5, requêtes                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `adapter-fs`     | Scan, watch (`notify`), corbeille (`trash`), `settings.json`, écriture ZIP                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `presentation`   | DTO exportés en TS (`ts-rs`), erreurs sérialisables, relais d'événements                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `desktop`        | Coquille Tauri : commandes IPC, fenêtres main + overlay, tray, raccourci global, autostart, single-instance, dialogues, updater                                                                                                                                                                                                                                                                                                                                                                                                                  |

## Correspondances Electron → Tauri

| Electron                                                | Tauri / Rust                                                                              |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `ipcMain.handle` + `preload.ts`                         | `#[tauri::command]` + `invoke()` ; `electronAPI` remplacé par un bridge `@tauri-apps/api` |
| `webContents.send`                                      | `Emitter::emit` / `listen()`                                                              |
| `utilityProcess` (collections)                          | Thread Rust + canal (`std::sync::mpsc`)                                                   |
| `better-sqlite3`                                        | `rusqlite` (`bundled`, FTS5 inclus)                                                       |
| `fs.watch` récursif                                     | `notify` (+ debouncer)                                                                    |
| `shell.trashItem` / `showItemInFolder` / `openExternal` | `trash` / `tauri-plugin-opener`                                                           |
| `dialog.*`                                              | `tauri-plugin-dialog`                                                                     |
| `globalShortcut`                                        | `tauri-plugin-global-shortcut`                                                            |
| `setLoginItemSettings`                                  | `tauri-plugin-autostart` (`--hidden`)                                                     |
| `requestSingleInstanceLock`                             | `tauri-plugin-single-instance`                                                            |
| `Tray` / `Menu`                                         | `tauri::tray` (feature `tray-icon`)                                                       |
| `backgroundMaterial: 'acrylic'`                         | `window-vibrancy::apply_acrylic`                                                          |
| `-webkit-app-region: drag`                              | `data-tauri-drag-region` (tabs-bar, overlay)                                              |
| `webUtils.getPathForFile` (drop)                        | `onDragDropEvent` de la webview (chemins natifs)                                          |
| électron-builder NSIS                                   | bundler NSIS Tauri, `fileAssociations` `.md`                                              |

## Dépendances Rust retenues

| Crate                                                                                 | Pourquoi                                                          |
| ------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `tauri` 2.12, `tauri-build`                                                           | Socle (Tauri 3 n'existe qu'en alpha)                              |
| `tauri-plugin-{dialog,opener,global-shortcut,autostart,single-instance,updater}`      | Plugins officiels, remplacent les API Electron équivalentes       |
| `window-vibrancy`                                                                     | Acrylique de l'overlay (maintenu par l'équipe Tauri)              |
| `rusqlite` (`bundled`)                                                                | SQLite + FTS5, synchrone comme better-sqlite3                     |
| `notify`                                                                              | Surveillance récursive des collections (debounce de 250 ms en std) |
| `trash`                                                                               | Corbeille Windows                                                 |
| `zip` (deflate)                                                                       | Export ZIP (remplace l'écrivain maison)                           |
| `serde`, `serde_json`                                                                 | Réglages, IPC                                                     |
| `ts-rs`                                                                               | Types TS générés                                                  |
| `sys-locale`                                                                          | Locale de l'OS au premier lancement (ajoutée en cours de route)   |
| `thiserror`, `anyhow`, `tracing`, `tracing-subscriber`                                | Erreurs, logs                                                     |

Écarts au plan : `notify-debouncer-full` et `tauri-plugin-process` se sont révélés inutiles ; `sys-locale` remplace `app.getLocale()` d'Electron.

Côté npm : `@tauri-apps/cli` (racine), `@tauri-apps/api` (frontend) ; `electron`, `electron-builder`, `@electron/rebuild`, `better-sqlite3`, `wait-on`, `concurrently` retirés. Le dépôt est en workspaces npm (`frontend/`).

## Étapes réalisées (un commit chacune)

1. `refactor: déplacer le renderer Angular dans frontend/`
2. `build(tauri): initialiser le workspace src-tauri`
3. `feat(domain): porter les règles pures du main process`
4. `feat(app): cas d'usage, ports et socle platform` (avant les adaptateurs, qui implémentent ses ports)
5. `feat(sqlite): porter la base de notes, dossiers et collections`
6. `feat(fs): collections, réglages et exports sur le disque`
7. `feat(presentation): DTO, erreurs IPC et bindings TypeScript`
8. `feat(desktop): coquille Tauri (fenêtres, tray, raccourci, mises à jour)`
9. `refactor(frontend): remplacer electronAPI par le bridge Tauri`
10. `build: retirer Electron`
11. `ci: build et release Tauri` + README

Tests : 219 tests Rust (spécification portée depuis les tests TS du main process) ; les tests TS restants (299) ne couvrent plus que le renderer.

## Points d'attention

- **Glisser-déposer depuis l'Explorateur (régression)** : le glisser-déposer HTML5 du renderer (onglets, arborescence) exige de désactiver le gestionnaire de dépôt de WebView2 (`disable_drag_drop_handler`) ; un fichier déposé n'a alors plus de chemin disque. Le dépôt de dossiers / `.md` sur la fenêtre est retiré ; l'ouverture depuis l'Explorateur (association `.md`) et le menu *Ouvrir* restent.
- **Fond flouté de l'overlay** : la capture `desktopCapturer` disparaît ; sans acrylique, panneau translucide uni.
- **Sonde acrylique** (économiseur de batterie, transparence) : portée telle quelle via `powershell.exe` (`std::process::Command`), sans `unsafe`.
- **Dossier manquant** : une collection dont le dossier a disparu émet désormais l'événement `error` (état « déplacé » du renderer) ; Electron rendait un arbre vide.
- **localStorage** (onglets ouverts, état de la sidebar, vue d'accueil) : l'origine change, il sera vide au premier lancement Tauri. La base SQLite et `settings.json` (`%APPDATA%/everything-note`) sont repris.
- **Utilisateurs Electron existants** :
  - hook NSIS ([hooks.nsh](../../src-tauri/crates/desktop/windows/hooks.nsh)) : désinstalle en silence la version Electron (clé `34a98de4-…`, UUID v5 de l'appId), données conservées ;
  - `latest.json` hybride ([update-manifest.mjs](../../scripts/update-manifest.mjs)) : champs Tauri (`platforms`) + champs Electron (`path`, `url`, `sha512`) pointant sur le même installeur. Le plugin Tauri tolère les champs inconnus (vérifié dans ses sources).
  - Electron lance l'installeur avec `/S --updated --force-run` : l'option de relance de Tauri est `/R`, l'app ne redémarre donc pas seule après cette unique mise à jour.
- **Exécutable** : `EverythingNote.exe` (`mainBinaryName`), distinct de l'exécutable Electron que son désinstalleur ferme.
- **Clé de signature des mises à jour** : générée sans mot de passe dans `%USERPROFILE%/.tauri/everything-note.key` (publique dans `tauri.conf.json`). À stocker dans le secret GitHub `TAURI_SIGNING_PRIVATE_KEY` (contenu du fichier) ; `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` vide. La perdre empêche toute mise à jour future.
- **Version et release** : `tauri.conf.json` lit `package.json`, tamponné par la CI le temps du build (jamais commité). Chaque fusion sur `main` publie une release dont la version est déduite des commits depuis le dernier tag (voir « Releases » du README), sur le modèle de `quark`. Le premier passage donnera `0.1.0` (la branche contient des `feat:`) : c'est ce qui fait proposer la version Tauri aux utilisateurs d'Electron (0.0.11).
- **Lint du renderer** : 5 erreurs préexistantes sur `main` (`editor.component.ts`, `markdown-decorations.ts`), non corrigées ici.
