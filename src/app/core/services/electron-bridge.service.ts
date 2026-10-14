import { Injectable } from '@angular/core';
import { Note } from '../models/note.model';
import { AppSettings, SettingsPatch, SettingsUpdateResult } from '../models/settings.model';

/** Raw row shape returned by the Electron main process (snake_case from SQLite). */
interface NoteRow {
  id: number;
  title: string;
  content: string;
  tags: string | string[];
  created_at?: string;
  createdAt?: string;
  updated_at?: string;
  updatedAt?: string;
}

interface ElectronAPI {
  minimize: () => Promise<void>;
  maximize: () => Promise<void>;
  close: () => Promise<void>;
  isMaximized: () => Promise<boolean>;
  getAllNotes: () => Promise<NoteRow[]>;
  getNoteById: (id: number) => Promise<NoteRow | null>;
  createNote: (title: string, content: string, tags?: string[]) => Promise<NoteRow>;
  updateNote: (id: number, data: { title?: string; content?: string; tags?: string }) => Promise<NoteRow | null>;
  deleteNote: (id: number) => Promise<boolean>;
  searchNotes: (query: string) => Promise<NoteRow[]>;
  getNotesByTag: (tag: string) => Promise<NoteRow[]>;
  getAllTags: () => Promise<string[]>;
  openExternal: (url: string) => Promise<void>;
  getSettings: () => Promise<AppSettings>;
  setSettings: (patch: SettingsPatch) => Promise<SettingsUpdateResult>;
  hideOverlay: () => Promise<void>;
  onOverlayShown: (callback: () => void) => () => void;
  onNotesChanged: (callback: () => void) => () => void;
  onSettingsChanged: (callback: (settings: AppSettings) => void) => () => void;
  onOpenSettings: (callback: () => void) => () => void;
}

function mapRow(row: NoteRow): Note {
  return {
    id: row.id,
    title: row.title,
    content: row.content,
    tags: typeof row.tags === 'string' ? JSON.parse(row.tags) : (row.tags ?? []),
    createdAt: row.created_at ?? row.createdAt ?? '',
    updatedAt: row.updated_at ?? row.updatedAt ?? '',
  };
}

@Injectable({ providedIn: 'root' })
export class ElectronBridgeService {
  private readonly isElectron = !!((window as unknown as Record<string, unknown>)['electronAPI']);
  private readonly api: ElectronAPI | null =
    ((window as unknown as Record<string, unknown>)['electronAPI'] as ElectronAPI | undefined) ?? null;

  get inElectron(): boolean { return this.isElectron; }

  // Window
  minimize(): Promise<void> { return this.api?.minimize() ?? Promise.resolve(); }
  maximize(): Promise<void> { return this.api?.maximize() ?? Promise.resolve(); }
  close(): Promise<void> { return this.api?.close() ?? Promise.resolve(); }
  isMaximized(): Promise<boolean> { return this.api?.isMaximized() ?? Promise.resolve(false); }

  // Notes
  async getAllNotes(): Promise<Note[]> {
    if (!this.api) return [];
    const rows = await this.api.getAllNotes();
    return rows.map(mapRow);
  }

  async getNoteById(id: number): Promise<Note | null> {
    if (!this.api) return null;
    const row = await this.api.getNoteById(id);
    return row ? mapRow(row) : null;
  }

  async createNote(title: string, content: string, tags: string[] = []): Promise<Note> {
    if (!this.api) throw new Error('Not in Electron');
    const row = await this.api.createNote(title, content, tags);
    return mapRow(row);
  }

  async updateNote(id: number, data: { title?: string; content?: string; tags?: string[] }): Promise<Note | null> {
    if (!this.api) return null;
    const payload = {
      ...data,
      tags: data.tags !== undefined ? JSON.stringify(data.tags) : undefined,
    };
    const row = await this.api.updateNote(id, payload);
    return row ? mapRow(row) : null;
  }

  async deleteNote(id: number): Promise<boolean> {
    if (!this.api) return false;
    return this.api.deleteNote(id);
  }

  async searchNotes(query: string): Promise<Note[]> {
    if (!this.api) return [];
    const rows = await this.api.searchNotes(query);
    return rows.map(mapRow);
  }

  async getNotesByTag(tag: string): Promise<Note[]> {
    if (!this.api) return [];
    const rows = await this.api.getNotesByTag(tag);
    return rows.map(mapRow);
  }

  async getAllTags(): Promise<string[]> {
    if (!this.api) return [];
    return this.api.getAllTags();
  }

  // Settings
  async getSettings(): Promise<AppSettings | null> {
    return this.api ? this.api.getSettings() : null;
  }

  async setSettings(patch: SettingsPatch): Promise<SettingsUpdateResult | null> {
    return this.api ? this.api.setSettings(patch) : null;
  }

  // Overlay
  hideOverlay(): Promise<void> { return this.api?.hideOverlay() ?? Promise.resolve(); }

  /** Event subscriptions return an unsubscribe function (no-op outside Electron). */
  onOverlayShown(callback: () => void): () => void {
    return this.api?.onOverlayShown(callback) ?? (() => undefined);
  }

  onNotesChanged(callback: () => void): () => void {
    return this.api?.onNotesChanged(callback) ?? (() => undefined);
  }

  onSettingsChanged(callback: (settings: AppSettings) => void): () => void {
    return this.api?.onSettingsChanged(callback) ?? (() => undefined);
  }

  onOpenSettings(callback: () => void): () => void {
    return this.api?.onOpenSettings(callback) ?? (() => undefined);
  }
}
