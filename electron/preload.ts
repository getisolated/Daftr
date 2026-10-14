import { contextBridge, ipcRenderer } from 'electron';

function subscribe(channel: string, callback: (payload?: any) => void): () => void {
  const listener = (_event: unknown, payload?: unknown): void => callback(payload);
  ipcRenderer.on(channel, listener);
  return () => { ipcRenderer.removeListener(channel, listener); };
}

const api = {
  // Window controls
  minimize: () => ipcRenderer.invoke('window:minimize'),
  maximize: () => ipcRenderer.invoke('window:maximize'),
  close: () => ipcRenderer.invoke('window:close'),
  isMaximized: () => ipcRenderer.invoke('window:isMaximized'),

  // Notes
  getAllNotes: () => ipcRenderer.invoke('notes:getAll'),
  getNoteById: (id: number) => ipcRenderer.invoke('notes:getById', id),
  createNote: (title: string, content: string, tags?: string[]) =>
    ipcRenderer.invoke('notes:create', title, content, tags),
  updateNote: (id: number, data: { title?: string; content?: string; tags?: string }) =>
    ipcRenderer.invoke('notes:update', id, data),
  deleteNote: (id: number) => ipcRenderer.invoke('notes:delete', id),
  searchNotes: (query: string) => ipcRenderer.invoke('notes:search', query),
  getNotesByTag: (tag: string) => ipcRenderer.invoke('notes:byTag', tag),
  getAllTags: () => ipcRenderer.invoke('notes:allTags'),

  // Settings
  getSettings: () => ipcRenderer.invoke('settings:get'),
  setSettings: (patch: Record<string, unknown>) => ipcRenderer.invoke('settings:set', patch),

  // Overlay
  hideOverlay: () => ipcRenderer.invoke('overlay:hide'),

  // Events pushed by the main process; each returns an unsubscribe function
  onOverlayShown: (callback: () => void) => subscribe('overlay:shown', callback),
  onNotesChanged: (callback: () => void) => subscribe('notes:changed', callback),
  onSettingsChanged: (callback: (settings: unknown) => void) => subscribe('settings:changed', callback),
  onOpenSettings: (callback: () => void) => subscribe('settings:open', callback),

  // Shell
  openExternal: (url: string) => ipcRenderer.invoke('shell:openExternal', url),
};

contextBridge.exposeInMainWorld('electronAPI', api);
