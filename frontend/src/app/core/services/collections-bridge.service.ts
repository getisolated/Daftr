import { Injectable } from '@angular/core';
import type { CollectionDto } from '../bindings/CollectionDto';
import { CollectionFileRef, CollectionKind, CollectionsEvent } from '../models/collection.model';
import { call, inDesktop, subscribe } from './desktop-api';

/** Raw row of the `collections` table (snake_case from SQLite). */
export type CollectionRow = Pick<CollectionDto, 'path' | 'kind' | 'display_name' | 'sort_order'>;

const NOT_IN_DESKTOP = 'Not in the desktop app';

/** Access to the collections of the main process (disk folders / files). Outside the desktop app everything is empty. */
@Injectable({ providedIn: 'root' })
export class CollectionsBridgeService {
  private readonly available = inDesktop();

  list(): Promise<CollectionRow[]> { return this.when([], () => call('collections_list')); }
  pick(kind: CollectionKind): Promise<CollectionRow[]> { return this.when([], () => call('collections_pick', { kind })); }
  remove(root: string): Promise<boolean> { return this.when(false, () => call('collections_remove', { root })); }
  rename(root: string, name: string): Promise<CollectionRow | null> { return this.when(null, () => call('collections_rename', { root, name })); }
  relocate(root: string): Promise<CollectionRow | null> { return this.when(null, () => call('collections_relocate', { root })); }
  load(root: string): Promise<boolean> { return this.when(false, () => call('collections_load', { root })); }
  read(ref: CollectionFileRef): Promise<string> { return this.when('', () => call('collections_read', { ...ref })); }
  write(ref: CollectionFileRef, content: string): Promise<boolean> { return this.when(false, () => call('collections_write', { ...ref, content })); }
  createFile(root: string, folder: string, name: string): Promise<string> { return this.required(() => call('collections_create_file', { root, folder, name })); }
  renamePath(ref: CollectionFileRef, name: string): Promise<string> { return this.required(() => call('collections_rename_path', { ...ref, name })); }
  movePath(ref: CollectionFileRef, folder: string): Promise<string> { return this.required(() => call('collections_move_path', { ...ref, folder })); }
  trash(ref: CollectionFileRef): Promise<boolean> { return this.when(false, () => call('collections_trash', { ...ref })); }
  reveal(ref: CollectionFileRef): Promise<boolean> { return this.when(false, () => call('collections_reveal', { ...ref })); }
  search(query: string): Promise<number> { return this.when(0, () => call('collections_search', { query })); }
  cancelSearch(requestId: number): Promise<boolean> { return this.when(false, () => call('collections_cancel_search', { requestId })); }
  requestGraph(root: string): Promise<boolean> { return this.when(false, () => call('collections_graph', { root })); }
  onEvent(callback: (event: CollectionsEvent) => void): () => void { return subscribe('collections:event', callback); }
  onOpen(callback: (refs: CollectionFileRef[]) => void): () => void { return subscribe('collections:open', callback); }

  /** The command's result, or `fallback` outside the desktop app. */
  private when<T>(fallback: T, command: () => Promise<T>): Promise<T> {
    return this.available ? command() : Promise.resolve(fallback);
  }

  /** The command's result; outside the desktop app the operation cannot be done at all. */
  private required<T>(command: () => Promise<T>): Promise<T> {
    return this.available ? command() : Promise.reject(new Error(NOT_IN_DESKTOP));
  }
}
