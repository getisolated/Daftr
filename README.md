# Daftr

A fast, keyboard-driven note-taking desktop app built with Angular, Rust (Tauri 2), and SQLite.

<p align="center">
  <img src="assets/logo.png" alt="Daftr" width="128" />
</p>

<p align="center">
  <img src="assets/screenshot.png" alt="Screenshot" width="800" />
</p>

## Features

- **Markdown editor** — CodeMirror 6 with live decorations (headings, bold, italic, code blocks, task lists)
- **Full-text search** — SQLite FTS5 for instant search across all notes
- **Command palette** — `F1` to search notes, commands, and tags
- **Tabs** — drag & drop reordering, pinning, session restore
- **Tables** — GFM tables render as real tables when the caret is elsewhere, and show their aligned markdown when it is inside: `Tab` / `Shift+Tab` jump between cells (`Tab` on the last cell adds a row), pipes are re-aligned on save and on blur, `/table` inserts an empty 2×2 table, and the right-click menu adds / deletes rows and columns and aligns a column. Spreadsheet (TSV) pastes become tables
- **Folders** — nested folders in a VS Code-like tree sidebar (collapsible, resizable, `Ctrl+Shift+E` to toggle): create / rename / delete, drag & drop notes and folders (drop on a folder to move into it, between two rows to position it: an insertion line shows where), note count per folder. Notes created on the fly land in the undeletable `Inbox` folder; the first-launch sample notes live in `Getting started`. 3-4 levels deep are recommended (a warning is shown beyond). `Ctrl+click` / `Shift+click` select several notes and folders: drag them together, `Delete` them (one confirmation) or *Move To…* a folder picked in a dialog. Right-click a folder to *Empty* it (its content is deleted, the folder kept, after a confirmation with the counts). *Color…* opens a color picker (saturation / hue, hex field, presets, recent colors): the icon, the label and a light background take that color
- **Backlinks & graph** — link notes with `[label](Folder/Note title)` (path autocompletion once `](` is typed, like in a code editor) or `note://ID` (palette: `Copy Note Link`). A collapsible panel under the editor lists the notes linking to the active one, with the surrounding text. The home page draws an Obsidian-like graph of the links (canvas + d3-force): zoom, pan, drag nodes, click to open, active note highlighted, notes coloured and gathered by folder (the deepest level of folders giving at most 10 colours), names drawn by importance without overlapping (zoom in to see more), spacing that grows with the number of notes, orphans on the outer ring, dashed nodes/links for deleted notes, filtered by the folder selected in the sidebar; above 500 notes it is limited to 2 links around the active note. Switch to the plain list with the `Graph / List` toggle. Links render as their label once the caret leaves them; `Ctrl+click` follows them (note, or web address in the browser). A breadcrumb above the note shows where it sits (the notes or its collection, the folders, the note; **Settings → Editor → Breadcrumbs**). Settings: line numbers (off by default), word wrap, outline (off by default) and a hidden editor scrollbar that shows again on hover
- **Tag system** — organize notes with tags, filter from the sidebar
- **Quick-note overlay** — a global shortcut (default `Ctrl+Alt+Q`, configurable) pops a small translucent, always-on-top chat-like window anywhere on screen; notes sent from it are tagged `quick-note` and gathered in the app in a single *Quick notes* document (sidebar entry, palette). Hovering a note shows Discord-like actions: copy, edit in place (`Escape` cancels, `Ctrl+Enter` or leaving the editor saves) and delete
- **System tray** — closing the window keeps the app running in the tray (right-click > Quit to exit); optional launch at Windows startup
- **Auto-update** — the app checks the latest GitHub release at startup then hourly (packaged Windows build only) and offers the update in a popup, with an entry in the sidebar above Settings; the installer signature (minisign) is verified before it runs. The `Automatic Updates` setting downloads in the background and installs when the app closes. Releases (tags `v*`) publish a `latest.json` manifest next to the installer (it also carries the fields read by the former Electron versions, so they update to this one)
- **Settings editor** — VS Code-like modal (`Ctrl+,`) with search, categories and reset to default
- **Themes** — Dark (default), Light, GitHub Light, Solarized Light, Violet Light and GitHub Dark Dimmed. **Settings → Appearance → Theme**, or `Ctrl+K Ctrl+T` for a VS Code-like picker: the arrow keys preview each theme live, `Enter` keeps it, `Escape` restores the previous one. `System` follows the Windows light/dark mode with the *Preferred Dark / Light Theme* settings. Chords (`Ctrl+K` then a second key) wait for the second key with no time limit; `Escape` or any other key cancels them. Every color is a CSS variable `--evn-*` defined per `data-theme` in `frontend/src/styles.scss` (the editor syntax colors too, so CodeMirror needs no per-theme extension): a new theme = a new `:root[data-theme='…']` block, registered in `theme.model.ts` (a test checks every block defines every token). The choice lives in `settings.json` and is mirrored in localStorage so an inline script in `index.html` applies it before the first paint. The quick-note overlay keeps its own dark translucent look
- **Formatting bar** — Word-like toolbar (styles, bold/italic/strike/code, blocks, lists) between the tabs and the editor, buttons reflect the formatting under the caret (read from the syntax tree) and never steal the editor focus. `Alt+T` toggles it; **Settings → Formatting Bar** turns it off for good and sets its size (Full / Compact), style (Pinned / Floating) and position (Top / Bottom). When the full bar does not fit along its side (measured: window size, sidebar, panels), each group (text, insert, blocks, lists) collapses into one drop-down: named when horizontal, an icon when vertical
- **Find & replace** — `Ctrl+F` / `Ctrl+H` open a panel in the right column (in place of the outline while open, like Word): match case / whole word / regular expression toggles (`Alt+C` / `Alt+W` / `Alt+R`), `3 of 12` counter, icon buttons, and the list of every match with its line and context (click to jump). `Enter` / `Shift+Enter` go to the next / previous match, `Enter` in the replace field replaces, `Ctrl+Alt+Enter` replaces all, `Escape` closes. CodeMirror's search state drives it (highlights, commands); its native panel is hidden
- **Preview** — GitHub-like rendering of a note (markdown-it, sanitized by DOMPurify): tables, task lists, footnotes, GitHub alerts (`> [!NOTE]`…), code colored with the editor's Lezer parsers, **Mermaid** diagrams (` ```mermaid `, loaded on demand, following the theme). The buttons at the top right of a note switch between *Source*, *side by side* (scrolls synchronized both ways) and *Preview*; `Ctrl+Shift+V` toggles the preview, `Ctrl+K V` the side-by-side view. **Settings → Editor → Open Notes In Preview** opens notes rendered. Local links pointing to nothing are shown in red, in the editor and in the preview
- **Copy & export** — right-click a note (tab, editor or sidebar) or use the palette: *Copy Content* (markdown), *Export as Markdown…*, *Export as HTML…* (standalone page rendered like the preview, GitHub light style, code colored and Mermaid diagrams drawn). Right-click a folder: *Export as ZIP…* (its notes as `.md` files, sub-folders kept). Links are exported as written
- **Collections** — open folders (e.g. the documentation of a repository) or markdown files of your disk from the *Collections* section of the sidebar, apart from your notes: only the `.md` files and the folders holding some are listed (build / dependency folders and hidden ones are skipped: **Settings → Collections → Excluded Folders**). Files open in tabs and are saved to the disk; a file changed by another program reloads, or shows a banner when you have unsaved edits. Create, rename, move (drag & drop inside a collection) and delete (to the Recycle Bin) files; relative links (`[x](./other.md)`) open with `Ctrl+click`. The palette search (`Ctrl+Shift+F`) also finds collection files by name and content. Two folders with the same name show their parent; hovering shows the disk path. `.md` files open with the app from the Explorer. A folder is only read once expanded (`N` next to its name until then, a spinner while it loads), so that the app starts without touching the disk; the palette searches the loaded ones. Scanning, watching and searching run on dedicated Rust threads: nothing blocks the app, and the notes never wait for the disk. The Collections and Tags sections are resized by dragging the line above them (double-click to reset)
- **Languages** — English and French. **Settings → General → Language**; on the first launch the language follows the OS locale (`fr*` → French, anything else → English). The change applies instantly, to the windows, the quick-note overlay, the tray menu and the native dialogs; the sample notes are created in the language chosen at that time. Texts live in typed dictionaries (`frontend/src/app/core/i18n/translations/{en,fr}/<area>.ts`, English is the reference, a test checks both define the same keys and placeholders); the main process has its own small one (`src-tauri/crates/domain/src/i18n.rs`). A new language = a value in `Language` (`language.model.ts`, mirrored in `src-tauri/crates/domain/src/i18n.rs`), a dictionary per area, and the main process messages
- **Keyboard-first** — every action has a shortcut (see below)
- **Local storage** — all data stays on your machine in a SQLite database

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Frontend | Angular 22 (standalone components, signals, OnPush) |
| Editor | CodeMirror 6 with custom markdown decorations |
| Desktop | Tauri 2 (Rust, WebView2), frameless window, custom title bar |
| Database | SQLite (rusqlite, bundled) with FTS5, WAL mode, cached prepared statements |
| Bindings | Rust → TypeScript types generated by ts-rs (`frontend/src/app/core/bindings`) |
| Build | Tauri bundler (NSIS installer), GitHub Actions CI/CD |

## Architecture

```
src-tauri/                   # Rust workspace (see docs/dev/migration-tauri.md)
  crates/
    platform/                # Generic base: logging, poison-tolerant locks, canonical paths
    domain/                  # Pure rules (std only): settings validation, overlay geometry, collection links/graph/search, export names, i18n
    app/                     # Use cases (notes, folders, collections, settings, export) and their ports (traits)
    adapter-sqlite/          # SQLite store: schema, idempotent migrations, FTS5 triggers, queries
    adapter-fs/              # settings.json, collection files, Recycle Bin, ZIP export, scan/watch/search thread
    presentation/            # DTOs exported to TypeScript (ts-rs), IPC errors, event names
    desktop/                 # Tauri shell: IPC commands, main + overlay windows, tray, global shortcut, autostart, updater
      tauri.conf.json        # Bundle, updater endpoint and public key, NSIS hook (removes the Electron version)
      capabilities/          # Window permissions granted to the renderer

frontend/src/app/
  core/
    i18n/            # I18nService (`t()`, `plural()`, current language), pure `translate()`, typed en/fr dictionaries per area
    services/        # NotesService, FoldersService, SidebarStateService (persisted in localStorage), TabsService, BacklinksService (backlinks + graph data), PaletteService, ShortcutsService, ThemeService (applies `data-theme`)
    models/          # TypeScript interfaces (Note, Folder, Tab, PaletteItem)
    utils/           # Pure helpers (folder-tree.util: tree building, move validation, sidebar width; note-links.util: `note://` parsing, backlinks, graph)
  features/
    editor/          # CodeMirror markdown editor with auto-save
    backlinks/       # Backlinks panel under the editor
    graph/           # Graph view (home page)
    sidebar/         # Folder tree (drag & drop, inline rename, context menu), quick notes, tags, settings button
    home/            # Note list of the selected folder / tag, context menu
    tabs/            # Tab bar with drag & drop, pinning, window controls
    palette/         # Command/search/tag palette (F1 / Ctrl+Shift+F)
    overlay/         # Quick-note overlay root component (bootstrapped in the `overlay` window)
    settings/        # VS Code-like settings modal (Ctrl+,), driven by settings-definitions.ts
  shared/
    codemirror/      # Editor highlight style (colors from theme tokens), markdown decorations, widgets
    context-menu/    # Reusable context menu component
    ui/              # Themed checkbox, select, confirm dialog, focus-select directive
```

## Keyboard Shortcuts

Every in-app shortcut below is a default and can be rebound in **Settings → Keyboard Shortcuts** (click a shortcut, press the new combination; `↺` restores the default). A combination already used by another action is refused. Overrides are stored in `settings.json` (`appShortcuts`); the list of rebindable actions is `APP_SHORTCUTS` in `frontend/src/app/core/models/app-shortcuts.model.ts` — add an entry there, then bind a handler with `ShortcutsService.bindAction(id, …)`. `Esc` (close dialogs) and `Ctrl+R` (blocked reload) are fixed.

| Shortcut | Action |
|----------|--------|
| `F1` | Command palette |
| `Ctrl+N` | New note |
| `Ctrl+Shift+E` | Toggle sidebar |
| `Ctrl+K` `Ctrl+T` (chord) | Color theme picker (live preview) |
| `Ctrl+W` | Close tab |
| `Ctrl+F` | Find in the note (panel on the right, with the list of matches) |
| `Ctrl+H` | Find & replace in the note |
| `Ctrl+Shift+V` | Toggle the preview |
| `Ctrl+K` `V` (chord) | Toggle the preview to the side |
| `Ctrl+Shift+F` | Search all notes |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |
| `Ctrl+S` | Save note |
| `Ctrl+B` / `Ctrl+I` / `Ctrl+Shift+K` | Bold / italic / link |
| `Alt+P` | Pin/unpin tab |
| `Alt+Z` | Toggle line wrap |
| `Alt+W` | Toggle wide mode |
| `Alt+T` | Toggle formatting bar |
| `Alt+Ctrl+1-9` | Jump to tab by index |
| `Ctrl+,` | Settings |
| `Ctrl+Alt+Q` (global, configurable) | Toggle the quick-note overlay (or show the app, see settings) |

In the overlay: `Enter` sends (`Shift+Enter` = new line) — or, if configured, `Enter` = new line and `Ctrl+Enter` sends. `Esc` or a click outside hides it; the draft is kept.

## Getting Started

### Prerequisites

- Node.js 24+ and npm
- Rust stable (1.90+), with the MSVC toolchain
- WebView2 (included in Windows 11)

### Install & Run

```bash
# Install dependencies (npm workspaces: the root holds the Tauri CLI, frontend/ the Angular app)
npm install

# Start in development mode (Angular dev server + Tauri window, Rust rebuilt on change)
npm run dev

# Tests: Angular (vitest) and Rust (also regenerates the TypeScript bindings)
npm test
npm run test:rust
```

### Build for Production

```bash
# Build Angular + Rust in release, then the NSIS installer (src-tauri/target/release/bundle/nsis)
# The update signature needs the private key: TAURI_SIGNING_PRIVATE_KEY (path or content)
npm run build
```

### Releases

Every merge on `main` publishes a release, with no manual step (`.github/workflows/release.yml`):

1. The version comes from the commits since the last `vX.Y.Z` tag ([Conventional Commits](https://www.conventionalcommits.org/)): `type!:` or `BREAKING CHANGE` bumps the major, `feat:` the minor, anything else the patch. A PR merged by *squash* counts as one commit whose title is the PR title, so **the title of the PR decides the bump** (`feat(scope): …`, `fix: …`).
2. The checks of `ci.yml` (Angular tests, `cargo fmt`, `clippy`, Rust tests) must pass first; the same workflow also runs on every pull request.
3. The version is stamped into `package.json` for the build only (it is never committed: the tag carries it), then the installer, its update signature and `latest.json` are published with the tag.

A commit message containing `[skip release]` publishes nothing, and so does a push with no commit since the last tag. Required secrets: `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty if the key has none). The `version` in `package.json` is only used until a first tag exists: local builds keep it.

## License

MIT
