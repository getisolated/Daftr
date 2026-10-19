# Migration Electron → Rust + Tauri 2

Branche `NF/refactor/tauri-main-process-261019`, partie de `main` (475f7d2).
Renderer Angular conservé ; le main process ([electron/](../../electron/), ~3000 lignes) est réécrit en Rust.

## Décisions prises

| Sujet                 | Choix                                                                                                          |
| --------------------- | -------------------------------------------------------------------------------------------------------------- |
| Arborescence          | Angular déplacé dans `frontend/` (`git mv`), Rust dans `src-tauri/`                                            |
| Données               | Même dossier qu'Electron : `%APPDATA%/Everything Note/` (`evnote.db`, `settings.json`), schéma SQLite inchangé |
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
| `domain`         | Pur, std seul : règles settings ([settings-schema.ts](../../electron/settings-schema.ts)), géométrie overlay ([overlay-geometry.ts](../../electron/overlay-geometry.ts)), versions ([update-manifest.ts](../../electron/update-manifest.ts)), noms/couleurs/FTS/LIKE/tags ([database.ts](../../electron/database.ts)), liens + graphe + recherche de collections ([collections-scan.ts](../../electron/collections-scan.ts)), noms d'export + ZIP paths ([export.ts](../../electron/export.ts)), textes i18n ([i18n.ts](../../electron/i18n.ts)) |
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

## Dépendances Rust à ajouter (à valider)

| Crate                                                                                    | Pourquoi                                                                                                                 |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `tauri` 2, `tauri-build`                                                                 | Socle                                                                                                                    |
| `tauri-plugin-{dialog,opener,global-shortcut,autostart,single-instance,updater,process}` | Plugins officiels, remplacent les API Electron équivalentes                                                              |
| `window-vibrancy`                                                                        | Acrylic de l'overlay (maintenu par l'équipe Tauri)                                                                       |
| `rusqlite` (`bundled`)                                                                   | SQLite + FTS5, synchrone comme better-sqlite3                                                                            |
| `notify`, `notify-debouncer-full`                                                        | Watch récursif des collections                                                                                           |
| `trash`                                                                                  | Corbeille Windows                                                                                                        |
| `zip` (deflate)                                                                          | Remplace l'écrivain ZIP maison ([zip.ts](../../electron/zip.ts)) ; alternative : porter l'écrivain avec `flate2` + crc32 |
| `serde`, `serde_json`                                                                    | Settings, IPC                                                                                                            |
| `ts-rs`                                                                                  | Types TS générés                                                                                                         |
| `thiserror` (libs), `anyhow` (desktop), `tracing`, `tracing-subscriber`                  | Erreurs, logs                                                                                                            |

Côté npm : `@tauri-apps/cli`, `@tauri-apps/api`, `@tauri-apps/plugin-*` ; retrait d'`electron`, `electron-builder`, `@electron/rebuild`, `better-sqlite3`, `wait-on`, `concurrently`.

## Étapes (un commit chacune)

1. `refactor: déplacer le renderer Angular dans frontend/`
2. `build(tauri): initialiser le workspace src-tauri`
3. `feat(domain): porter les règles pures du main process` (+ tests Rust portés depuis `tests/*` qui importent `electron/`)
4. `feat(sqlite): porter la base de notes, dossiers et collections`
5. `feat(app): cas d'usage et ports`
6. `feat(fs): collections, settings et export sur disque`
7. `feat(presentation): DTO et bindings TypeScript`
8. `feat(desktop): coquille Tauri (fenêtres, tray, raccourci, updater)`
9. `refactor(frontend): remplacer electronAPI par le bridge Tauri`
10. `build: retirer Electron`
11. `ci: build et release Tauri` + README

## Points d'attention

- **Fond flouté de l'overlay** : la capture `desktopCapturer` disparaît ; repli sur le panneau translucide uni (déjà géré côté renderer quand le backdrop est `null`), l'événement `overlay:backdrop` est supprimé.
- **Sonde acrylic** (économiseur de batterie, transparence) : portée telle quelle via `powershell.exe` (`std::process::Command`), sans `unsafe` ni crate `windows`.
- **localStorage** (onglets ouverts, état de la sidebar, vue d'accueil) : l'origine change (`http://tauri.localhost`), il sera vide au premier lancement Tauri. Les données SQLite et `settings.json` sont conservées.
- **Utilisateurs Electron existants** : l'installeur Tauri n'installe pas au même endroit ; un hook NSIS (`NSIS_HOOK_PREINSTALL`) désinstalle silencieusement la version Electron.
- **`latest.json` hybride** (temporaire) : un seul manifeste lu par les deux updaters, chacun ignorant les champs de l'autre. Il pointe sur le même installeur NSIS Tauri :
  - champs Electron `version`, `path`, `url`, `sha512` (vérifiés par [update-manifest.ts](../../electron/update-manifest.ts)) ;
  - champs Tauri `version`, `notes`, `pub_date`, `platforms.windows-x86_64.{url, signature}` (signature minisign du `.sig`).

  L'app Electron propose donc la version Tauri comme une mise à jour normale. Les champs Electron seront retirés quand plus aucune 0.0.x ne sera en circulation.
  À vérifier : Electron lance l'installeur avec `/S --updated --force-run`. Si `--force-run` n'est pas reconnu par l'installeur Tauri, l'app ne redémarre pas après cette unique mise à jour.
- **Secrets CI** : `TAURI_SIGNING_PRIVATE_KEY` (+ mot de passe) à créer dans le dépôt GitHub.
- **Version** : `tauri.conf.json` lit `../package.json` (source unique, la CI continue de tamponner le tag).
