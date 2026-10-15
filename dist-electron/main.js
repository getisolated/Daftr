"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
const main_1 = require("electron/main");
const common_1 = require("electron/common");
const path = __importStar(require("path"));
const database_1 = require("./database");
const global_shortcut_1 = require("./global-shortcut");
const overlay_1 = require("./overlay");
const renderer_loader_1 = require("./renderer-loader");
const settings_1 = require("./settings");
const settings_schema_1 = require("./settings-schema");
const startup_1 = require("./startup");
const tray_1 = require("./tray");
const updater_1 = require("./updater");
const isDev = process.env['NODE_ENV'] === 'development';
let mainWindow = null;
// Closing the window only hides it to the tray; a real quit comes from the tray menu (or the OS)
let isQuitting = false;
/** Window color shown before the renderer paints: matches the theme so a light theme does not flash dark. */
function windowBackgroundColor() {
    const { theme } = (0, settings_1.getSettings)();
    const isDark = theme === 'system' ? main_1.nativeTheme.shouldUseDarkColors : theme === 'dark';
    return isDark ? '#1e1e1e' : '#ffffff';
}
function createWindow(showOnReady = true) {
    mainWindow = new main_1.BrowserWindow({
        show: false,
        width: 1280,
        height: 800,
        minWidth: 600,
        minHeight: 400,
        frame: false,
        backgroundColor: windowBackgroundColor(),
        titleBarStyle: 'hidden',
        icon: path.join(__dirname, '../assets/logo.ico'),
        webPreferences: {
            preload: path.join(__dirname, 'preload.js'),
            contextIsolation: true,
            nodeIntegration: false,
            sandbox: true,
        },
    });
    (0, renderer_loader_1.loadRenderer)(mainWindow);
    if (isDev) {
        mainWindow.webContents.openDevTools({ mode: 'detach' });
    }
    if (showOnReady) {
        mainWindow.once('ready-to-show', () => mainWindow?.show());
    }
    mainWindow.on('close', (event) => {
        if (!isQuitting) {
            event.preventDefault();
            mainWindow?.hide();
        }
    });
    // Windows logoff/shutdown: let the window really close instead of blocking the session end
    mainWindow.on('session-end', () => { isQuitting = true; });
    mainWindow.on('closed', () => { mainWindow = null; });
}
function showMainWindow() {
    if (!mainWindow) {
        createWindow();
        return;
    }
    if (mainWindow.isMinimized())
        mainWindow.restore();
    mainWindow.show();
    mainWindow.focus();
}
function openSettings() {
    showMainWindow();
    mainWindow?.webContents.send('settings:open');
}
function quitApp() {
    isQuitting = true;
    main_1.app.quit();
}
function onGlobalShortcut() {
    if ((0, settings_1.getSettings)().shortcutMode === 'overlay') {
        (0, overlay_1.toggleOverlay)();
    }
    else {
        (0, overlay_1.hideOverlay)();
        showMainWindow();
    }
}
// A second launch (start menu, login item...) just brings the running instance forward
const isPrimaryInstance = main_1.app.requestSingleInstanceLock();
if (!isPrimaryInstance) {
    main_1.app.quit();
}
else {
    main_1.app.on('second-instance', () => showMainWindow());
}
main_1.app.whenReady().then(() => {
    // app.quit() is asynchronous: the duplicate instance must not create its windows/tray meanwhile
    if (!isPrimaryInstance)
        return;
    (0, database_1.initDatabase)();
    const settings = (0, settings_1.loadSettings)();
    createWindow(!(0, startup_1.launchedHidden)());
    void (0, overlay_1.prepareOverlay)();
    (0, tray_1.createTray)({ openApp: showMainWindow, quickNote: overlay_1.showOverlay, openSettings, quit: quitApp });
    (0, startup_1.applyLaunchAtStartup)(settings.launchAtStartup);
    if (!(0, global_shortcut_1.registerGlobalShortcut)(settings.globalShortcut, onGlobalShortcut)) {
        console.error(`[Shortcut] Unable to register ${settings.globalShortcut}`);
    }
    (0, updater_1.startUpdater)();
    main_1.app.on('activate', () => {
        if (!mainWindow)
            createWindow();
    });
});
// Never quit on window close: the app lives in the tray
main_1.app.on('window-all-closed', () => { });
main_1.app.on('before-quit', () => {
    isQuitting = true;
    (0, database_1.closeDatabase)();
});
main_1.app.on('will-quit', () => {
    (0, updater_1.stopUpdater)();
    (0, global_shortcut_1.unregisterGlobalShortcuts)();
    (0, tray_1.destroyTray)();
});
// Wraps an IPC handler with try-catch to prevent unhandled exceptions
function safeHandle(channel, handler) {
    main_1.ipcMain.handle(channel, async (...args) => {
        try {
            return await handler(...args);
        }
        catch (err) {
            console.error(`[IPC] Error in ${channel}:`, err);
            throw err;
        }
    });
}
// ── Window controls ──────────────────────────────────────────────────────────
safeHandle('window:minimize', () => mainWindow?.minimize());
safeHandle('window:maximize', () => {
    if (mainWindow?.isMaximized())
        mainWindow.unmaximize();
    else
        mainWindow?.maximize();
});
safeHandle('window:close', () => mainWindow?.close());
safeHandle('window:isMaximized', () => mainWindow?.isMaximized() ?? false);
// ── Notes CRUD ───────────────────────────────────────────────────────────────
safeHandle('notes:getAll', () => database_1.noteOps.getAll());
safeHandle('notes:getById', (_e, id) => database_1.noteOps.getById(id) ?? null);
safeHandle('notes:create', (e, title, content, tags, folderId) => {
    const note = database_1.noteOps.create(title, content, Array.isArray(tags) ? tags.filter(t => typeof t === 'string') : [], typeof folderId === 'number' ? folderId : null);
    // Lets the other window (overlay <-> main app) refresh its list
    (0, overlay_1.broadcastToWindows)('notes:changed', undefined, e.sender);
    return note;
});
safeHandle('notes:update', (_e, id, data) => database_1.noteOps.update(id, data) ?? null);
safeHandle('notes:move', (_e, id, folderId) => database_1.noteOps.moveToFolder(id, typeof folderId === 'number' ? folderId : null) ?? null);
safeHandle('notes:place', (_e, id, folderId, orderedIds) => database_1.noteOps.place(id, typeof folderId === 'number' ? folderId : null, parseIds(orderedIds)) ?? null);
safeHandle('notes:delete', (_e, id) => { database_1.noteOps.delete(id); return true; });
safeHandle('notes:search', (_e, query) => database_1.noteOps.search(query));
safeHandle('notes:byTag', (_e, tag) => database_1.noteOps.getByTag(tag));
safeHandle('notes:allTags', () => database_1.noteOps.getAllTags());
// ── Folders ──────────────────────────────────────────────────────────────────
function parseParentId(value) {
    return typeof value === 'number' ? value : null;
}
function parseIds(value) {
    return Array.isArray(value) ? value.filter((id) => typeof id === 'number') : [];
}
safeHandle('folders:getAll', () => database_1.folderOps.getAll());
safeHandle('folders:create', (_e, name, parentId) => database_1.folderOps.create(name, parseParentId(parentId)) ?? null);
safeHandle('folders:rename', (_e, id, name) => database_1.folderOps.rename(id, name) ?? null);
safeHandle('folders:move', (_e, id, parentId) => database_1.folderOps.move(id, parseParentId(parentId)) ?? null);
safeHandle('folders:place', (_e, id, parentId, orderedIds) => database_1.folderOps.place(id, parseParentId(parentId), parseIds(orderedIds)) ?? null);
safeHandle('folders:delete', (e, id, mode) => {
    // Anything but an explicit "deleteAll" keeps the notes: the safe choice if the payload is malformed
    const deletedNoteIds = database_1.folderOps.delete(id, mode === 'deleteAll' ? 'deleteAll' : 'moveToRoot');
    (0, overlay_1.broadcastToWindows)('notes:changed', undefined, e.sender);
    return deletedNoteIds;
});
// ── Settings & overlay ───────────────────────────────────────────────────────
safeHandle('settings:get', () => (0, settings_1.getSettings)());
safeHandle('settings:set', (e, patch) => {
    const before = (0, settings_1.getSettings)();
    // The position is owned by the main process: never take it from the renderer
    const candidate = (0, settings_schema_1.sanitizeSettings)({ ...before, ...patch, overlayPosition: before.overlayPosition }, before);
    if (candidate.globalShortcut !== before.globalShortcut &&
        !(0, global_shortcut_1.registerGlobalShortcut)(candidate.globalShortcut, onGlobalShortcut)) {
        return { ok: false, error: 'shortcut-unavailable', settings: before };
    }
    const next = (0, settings_1.updateSettings)(candidate);
    (0, overlay_1.applyOverlaySize)(before, next);
    if (next.launchAtStartup !== before.launchAtStartup) {
        (0, startup_1.applyLaunchAtStartup)(next.launchAtStartup);
    }
    if (next.autoUpdate !== before.autoUpdate) {
        (0, updater_1.onAutoUpdateChanged)();
    }
    (0, overlay_1.broadcastToWindows)('settings:changed', (0, settings_1.getSettings)(), e.sender);
    return { ok: true, settings: (0, settings_1.getSettings)() };
});
safeHandle('overlay:hide', () => (0, overlay_1.hideOverlay)());
// ── Updates ──────────────────────────────────────────────────────────────────
safeHandle('update:getInfo', () => (0, updater_1.getUpdateInfo)());
safeHandle('update:apply', () => (0, updater_1.applyUpdate)());
// ── Shell ─────────────────────────────────────────────────────────────────────
safeHandle('shell:openExternal', (_e, url) => {
    if (typeof url !== 'string' || !(url.startsWith('http://') || url.startsWith('https://'))) {
        throw new Error('Only http:// and https:// URLs are allowed');
    }
    return common_1.shell.openExternal(url);
});
