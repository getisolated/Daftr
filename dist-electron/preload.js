"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const renderer_1 = require("electron/renderer");
function subscribe(channel, callback) {
    const listener = (_event, payload) => callback(payload);
    renderer_1.ipcRenderer.on(channel, listener);
    return () => { renderer_1.ipcRenderer.removeListener(channel, listener); };
}
const api = {
    // Window controls
    minimize: () => renderer_1.ipcRenderer.invoke('window:minimize'),
    maximize: () => renderer_1.ipcRenderer.invoke('window:maximize'),
    close: () => renderer_1.ipcRenderer.invoke('window:close'),
    isMaximized: () => renderer_1.ipcRenderer.invoke('window:isMaximized'),
    // Notes
    getAllNotes: () => renderer_1.ipcRenderer.invoke('notes:getAll'),
    getNoteById: (id) => renderer_1.ipcRenderer.invoke('notes:getById', id),
    createNote: (title, content, tags, folderId) => renderer_1.ipcRenderer.invoke('notes:create', title, content, tags, folderId),
    updateNote: (id, data) => renderer_1.ipcRenderer.invoke('notes:update', id, data),
    moveNote: (id, folderId) => renderer_1.ipcRenderer.invoke('notes:move', id, folderId),
    deleteNote: (id) => renderer_1.ipcRenderer.invoke('notes:delete', id),
    searchNotes: (query) => renderer_1.ipcRenderer.invoke('notes:search', query),
    getNotesByTag: (tag) => renderer_1.ipcRenderer.invoke('notes:byTag', tag),
    getAllTags: () => renderer_1.ipcRenderer.invoke('notes:allTags'),
    // Folders
    getAllFolders: () => renderer_1.ipcRenderer.invoke('folders:getAll'),
    createFolder: (name, parentId) => renderer_1.ipcRenderer.invoke('folders:create', name, parentId),
    renameFolder: (id, name) => renderer_1.ipcRenderer.invoke('folders:rename', id, name),
    moveFolder: (id, parentId) => renderer_1.ipcRenderer.invoke('folders:move', id, parentId),
    deleteFolder: (id, mode) => renderer_1.ipcRenderer.invoke('folders:delete', id, mode),
    // Settings
    getSettings: () => renderer_1.ipcRenderer.invoke('settings:get'),
    setSettings: (patch) => renderer_1.ipcRenderer.invoke('settings:set', patch),
    // Overlay
    hideOverlay: () => renderer_1.ipcRenderer.invoke('overlay:hide'),
    // Events pushed by the main process; each returns an unsubscribe function
    onOverlayShown: (callback) => subscribe('overlay:shown', callback),
    onOverlayBackdrop: (callback) => subscribe('overlay:backdrop', callback),
    onNotesChanged: (callback) => subscribe('notes:changed', callback),
    onSettingsChanged: (callback) => subscribe('settings:changed', callback),
    onOpenSettings: (callback) => subscribe('settings:open', callback),
    // Shell
    openExternal: (url) => renderer_1.ipcRenderer.invoke('shell:openExternal', url),
};
renderer_1.contextBridge.exposeInMainWorld('electronAPI', api);
