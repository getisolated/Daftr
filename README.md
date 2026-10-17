# Everything Note

A fast, keyboard-driven note-taking desktop app built with Angular, Electron, and SQLite.

<p align="center">
  <img src="assets/logo.png" alt="Everything Note" width="128" />
</p>

<p align="center">
  <img src="assets/screenshot.png" alt="Screenshot" width="800" />
</p>

## Features

- **Markdown editor** — CodeMirror 6 with live decorations (headings, bold, italic, code blocks, task lists)
- **Full-text search** — SQLite FTS5 for instant search across all notes
- **Command palette** — `F1` to search notes, commands, and tags
- **Tabs** — drag & drop reordering, pinning, session restore. `Ctrl+E` opens the recent documents switcher (most recently used first, the previous one highlighted; `Ctrl+E` again while holding `Ctrl` moves down, releasing `Ctrl` opens). **Settings → Experimental → Tabs Style**: *Pills* (default), *Attached* (tabs joined to the page, like a browser), *Attached Reverse* (tabs hanging from the top of the window), *Recents* (no tabs: the current document centered in the title bar opens the switcher), *Trail* (the documents along time, the current one in the middle and older ones fading away), *Deck* (a pile of cards fanned out on hover) or *Dial* (a half circle of documents unfolding under the title bar, the most recent closest)
- **Tables** — GFM tables render as real tables when the caret is elsewhere, and show their aligned markdown when it is inside: `Tab` / `Shift+Tab` jump between cells (`Tab` on the last cell adds a row), pipes are re-aligned on save and on blur, `/table` inserts an empty 2×2 table, and the right-click menu adds / deletes rows and columns and aligns a column. Spreadsheet (TSV) pastes become tables
- **Folders** — nested folders in a VS Code-like tree sidebar (collapsible, resizable, `Ctrl+Shift+E` to toggle): create / rename / delete, drag & drop notes and folders (drop on a folder to move into it, between two rows to position it: an insertion line shows where), note count per folder. Notes created on the fly land in the undeletable `Inbox` folder; the first-launch sample notes live in `Getting started`. 3-4 levels deep are recommended (a warning is shown beyond). `Ctrl+click` / `Shift+click` select several notes and folders: drag them together, `Delete` them (one confirmation) or *Move To…* a folder picked in a dialog. Right-click a folder to *Empty* it (its content is deleted, the folder kept, after a confirmation with the counts). *Color…* opens a color picker (saturation / hue, hex field, presets, recent colors): the icon, the label and a light background take that color
- **Backlinks & graph** — link notes with `[label](Folder/Note title)` (path autocompletion once `](` is typed, like in a code editor) or `note://ID` (palette: `Copy Note Link`). A collapsible panel under the editor lists the notes linking to the active one, with the surrounding text. The home page draws an Obsidian-like graph of the links (canvas + d3-force): zoom, pan, drag nodes, click to open, active note highlighted, orphans on the outer ring, dashed nodes/links for deleted notes, filtered by the folder selected in the sidebar; above 500 notes it is limited to 2 links around the active note. Switch to the plain list with the `Graph / List` toggle. Links render as their label once the caret leaves them; `Ctrl+click` follows them (note, or web address in the browser). Settings: line numbers (off by default), word wrap, outline (off by default) and a hidden editor scrollbar that shows again on hover
- **Tag system** — organize notes with tags, filter from the sidebar
- **Quick-note overlay** — a global shortcut (default `Ctrl+Alt+N`, configurable) pops a small translucent, always-on-top chat-like window anywhere on screen; notes sent from it are tagged `quick-note` and gathered in the app in a single *Quick notes* document (sidebar entry, palette). Hovering a note shows Discord-like actions: copy, edit in place (`Escape` cancels, `Ctrl+Enter` or leaving the editor saves) and delete
- **System tray** — closing the window keeps the app running in the tray (right-click > Quit to exit); optional launch at Windows startup
- **Auto-update** — the app checks the latest GitHub release at startup then hourly (packaged Windows build only) and offers the update in a popup, with an entry in the sidebar above Settings; the installer is SHA-512 verified. The `Automatic Updates` setting downloads in the background and installs when the app closes. Releases (tags `v*`) publish a `latest.json` manifest next to the installer
- **Settings editor** — VS Code-like modal (`Ctrl+,`) with search, categories and reset to default
- **Themes** — Dark (default), Light, GitHub Light, Solarized Light, Violet Light and GitHub Dark Dimmed. **Settings → Appearance → Theme**, or `Ctrl+K Ctrl+T` for a VS Code-like picker: the arrow keys preview each theme live, `Enter` keeps it, `Escape` restores the previous one. `System` follows the Windows light/dark mode with the *Preferred Dark / Light Theme* settings. Chords (`Ctrl+K` then a second key) wait for the second key with no time limit; `Escape` or any other key cancels them. Every color is a CSS variable `--evn-*` defined per `data-theme` in `src/styles.scss` (the editor syntax colors too, so CodeMirror needs no per-theme extension): a new theme = a new `:root[data-theme='…']` block, registered in `theme.model.ts` (a test checks every block defines every token). The choice lives in `settings.json` and is mirrored in localStorage so an inline script in `index.html` applies it before the first paint. The quick-note overlay keeps its own dark translucent look
- **Formatting bar** — Word-like toolbar (styles, bold/italic/strike/code, blocks, lists) between the tabs and the editor, buttons reflect the formatting under the caret (read from the syntax tree) and never steal the editor focus. `Alt+T` toggles it; **Settings → Formatting Bar** turns it off for good and sets its size (Full / Compact), style (Pinned / Floating) and position (Top / Bottom)
- **Find & replace** — `Ctrl+F` / `Ctrl+H` open a panel in the right column (in place of the outline while open, like Word): match case / whole word / regular expression toggles (`Alt+C` / `Alt+W` / `Alt+R`), `3 of 12` counter, icon buttons, and the list of every match with its line and context (click to jump). `Enter` / `Shift+Enter` go to the next / previous match, `Enter` in the replace field replaces, `Ctrl+Alt+Enter` replaces all, `Escape` closes. CodeMirror's search state drives it (highlights, commands); its native panel is hidden
- **Preview** — GitHub-like rendering of a note (markdown-it, sanitized by DOMPurify): tables, task lists, footnotes, GitHub alerts (`> [!NOTE]`…), code colored with the editor's Lezer parsers, **Mermaid** diagrams (` ```mermaid `, loaded on demand, following the theme). The buttons at the top right of a note switch between *Source*, *side by side* (scrolls synchronized both ways) and *Preview*; `Ctrl+Shift+V` toggles the preview, `Ctrl+K V` the side-by-side view. **Settings → Editor → Open Notes In Preview** opens notes rendered. Local links pointing to nothing are shown in red, in the editor and in the preview
- **Copy & export** — right-click a note (tab, editor or sidebar) or use the palette: *Copy Content* (markdown), *Export as Markdown…*, *Export as HTML…* (standalone page rendered like the preview, GitHub light style, code colored and Mermaid diagrams drawn). Right-click a folder: *Export as ZIP…* (its notes as `.md` files, sub-folders kept). Links are exported as written
- **Collections** — open folders (e.g. the documentation of a repository) or markdown files of your disk from the *Collections* section of the sidebar, apart from your notes: only the `.md` files and the folders holding some are listed (build / dependency folders and hidden ones are skipped: **Settings → Collections → Excluded Folders**). Files open in tabs and are saved to the disk; a file changed by another program reloads, or shows a banner when you have unsaved edits. Create, rename, move (drag & drop inside a collection) and delete (to the Recycle Bin) files; relative links (`[x](./other.md)`) open with `Ctrl+click`. The palette search (`Ctrl+Shift+F`) also finds collection files by name and content. Two folders with the same name show their parent; hovering shows the disk path. Folders and `.md` files can be dropped on the window, and `.md` files open with the app from the Explorer. Click a collection to browse it on the home page: the graph shows the links between its files (relative markdown links, missing targets dashed; never across collections). Scanning, watching, searching and the graph run in a separate process (Electron utilityProcess): nothing blocks the app, and the notes never wait for the disk
- **Keyboard-first** — every action has a shortcut (see below)
- **Local storage** — all data stays on your machine in a SQLite database

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Frontend | Angular 22 (standalone components, signals, OnPush) |
| Editor | CodeMirror 6 with custom markdown decorations |
| Desktop | Electron 44 (frameless window, custom title bar) |
| Database | better-sqlite3 with FTS5, WAL mode, cached prepared statements |
| Build | electron-builder (NSIS installer), GitHub Actions CI/CD |

## Architecture

```
electron/
  main.ts            # Main process — window management, IPC handlers
  overlay.ts         # Quick-note overlay window (show/hide, position memory)
  overlay-geometry.ts# Pure helpers: default/saved overlay position, resize anchoring
  global-shortcut.ts # Global shortcut registration (keeps the previous one on failure)
  settings.ts        # settings.json persistence (userData)
  settings-schema.ts # Settings type, defaults, validation
  renderer-loader.ts # Loads the Angular bundle (#overlay selects the overlay root)
  tray.ts            # System tray icon & menu
  updater.ts         # Update check (hourly), verified download, silent install on exit/restart
  update-manifest.ts # Pure helpers: version comparison, manifest validation
  startup.ts         # Launch at startup (login item, started hidden with --hidden)
  preload.ts         # Context bridge (electronAPI)
  database.ts        # SQLite schema + migration (folders, notes.folder_id), FTS5 triggers, CRUD operations

src/app/
  core/
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
    overlay/         # Quick-note overlay root component (bootstrapped on #overlay)
    settings/        # VS Code-like settings modal (Ctrl+,), driven by settings-definitions.ts
  shared/
    codemirror/      # Editor highlight style (colors from theme tokens), markdown decorations, widgets
    context-menu/    # Reusable context menu component
    ui/              # Themed checkbox, select, confirm dialog, focus-select directive
```

## Keyboard Shortcuts

Every in-app shortcut below is a default and can be rebound in **Settings → Keyboard Shortcuts** (click a shortcut, press the new combination; `↺` restores the default). A combination already used by another action is refused. Overrides are stored in `settings.json` (`appShortcuts`); the list of rebindable actions is `APP_SHORTCUTS` in `src/app/core/models/app-shortcuts.model.ts` — add an entry there, then bind a handler with `ShortcutsService.bindAction(id, …)`. `Esc` (close dialogs) and `Ctrl+R` (blocked reload) are fixed.

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
| `Ctrl+E` | Recent documents switcher (again while holding `Ctrl`: next one) |
| `Ctrl+S` | Save note |
| `Ctrl+B` / `Ctrl+I` / `Ctrl+Shift+K` | Bold / italic / link |
| `Alt+P` | Pin/unpin tab |
| `Alt+Z` | Toggle line wrap |
| `Alt+W` | Toggle wide mode |
| `Alt+T` | Toggle formatting bar |
| `Alt+Ctrl+1-9` | Jump to tab by index |
| `Ctrl+,` | Settings |
| `Ctrl+Alt+N` (global, configurable) | Toggle the quick-note overlay (or show the app, see settings) |

In the overlay: `Enter` sends (`Shift+Enter` = new line) — or, if configured, `Enter` = new line and `Ctrl+Enter` sends. `Esc` or a click outside hides it; the draft is kept.

## Getting Started

### Prerequisites

- Node.js 20+
- npm

### Install & Run

```bash
# Install dependencies
npm install

# Rebuild native modules for Electron
npm run electron:rebuild

# Start in development mode (Angular dev server + Electron)
npm run dev
```

### Build for Production

```bash
# Build Angular + compile Electron TypeScript
npm run build

# Package as Windows installer (.exe)
npm run dist
```

## License

MIT
