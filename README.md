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
- **Tabs** — drag & drop reordering, pinning, session restore
- **Tag system** — organize notes with tags, filter from the sidebar
- **Quick-note overlay** — a global shortcut (default `Ctrl+Alt+N`, configurable) pops a small translucent, always-on-top chat-like window anywhere on screen; notes sent from it are tagged `overlay` and appear in the app
- **System tray** — closing the window keeps the app running in the tray (right-click > Quit to exit); optional launch at Windows startup
- **Settings editor** — VS Code-like modal (`Ctrl+,`) with search, categories and reset to default
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
  startup.ts         # Launch at startup (login item, started hidden with --hidden)
  preload.ts         # Context bridge (electronAPI)
  database.ts        # SQLite schema, FTS5 triggers, CRUD operations

src/app/
  core/
    services/        # NotesService, TabsService, PaletteService, ShortcutsService
    models/          # TypeScript interfaces (Note, Tab, PaletteItem)
  features/
    editor/          # CodeMirror markdown editor with auto-save
    home/            # Note list with tag filtering & context menu
    tabs/            # Tab bar with drag & drop, pinning, window controls
    palette/         # Command/search/tag palette (F1 / Ctrl+Shift+F)
    overlay/         # Quick-note overlay root component (bootstrapped on #overlay)
    settings/        # VS Code-like settings modal (Ctrl+,), driven by settings-definitions.ts
  shared/
    codemirror/      # VS Code Dark Modern theme, markdown decorations, widgets
    context-menu/    # Reusable context menu component
```

## Keyboard Shortcuts

Every in-app shortcut below is a default and can be rebound in **Settings → Keyboard Shortcuts** (click a shortcut, press the new combination; `↺` restores the default). A combination already used by another action is refused. Overrides are stored in `settings.json` (`appShortcuts`); the list of rebindable actions is `APP_SHORTCUTS` in `src/app/core/models/app-shortcuts.model.ts` — add an entry there, then bind a handler with `ShortcutsService.bindAction(id, …)`. `Esc` (close dialogs) and `Ctrl+R` (blocked reload) are fixed.

| Shortcut | Action |
|----------|--------|
| `F1` | Command palette |
| `Ctrl+N` | New note |
| `Ctrl+W` | Close tab |
| `Ctrl+F` | Find in editor |
| `Ctrl+Shift+F` | Search all notes |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |
| `Ctrl+S` | Save note |
| `Ctrl+B` / `Ctrl+I` / `Ctrl+K` | Bold / italic / link |
| `Alt+P` | Pin/unpin tab |
| `Alt+Z` | Toggle wide mode |
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
