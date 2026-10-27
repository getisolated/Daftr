import { Injectable, inject } from '@angular/core';
import { QUICK_NOTE_TAG } from '../models/settings.model';
import { retargetNoteLinks } from '../utils/note-links.util';
import { notePath, pathKey } from '../utils/note-paths.util';
import { FoldersService } from './folders.service';
import { NotesService } from './notes.service';

/** Idle time after the last title change before the links are rewritten: the title is typed letter by letter. */
const RENAME_DEBOUNCE_MS = 1500;
/** Longest stretch of work done without giving the UI a chance to run. */
const WORK_SLICE_MS = 8;

interface PendingRename {
  /** Title the links were written with: the one before the first change of the burst. */
  fromTitle: string;
  timer: ReturnType<typeof setTimeout>;
}

/** Keeps the `[label](Folder/Note title)` links valid when a note is retitled or moved, or a folder renamed or moved; all done in the background. */
@Injectable({ providedIn: 'root' })
export class LinkRenameService {
  private notes = inject(NotesService);
  private folders = inject(FoldersService);

  private readonly pending = new Map<number, PendingRename>();
  /** Rewrites run one after the other so two of them never write the same note at once. */
  private queue: Promise<void> = Promise.resolve();

  /** Called when the title of a note changed; the links to the previous title are fixed once the typing stops. */
  notifyTitleChanged(noteId: number, previousTitle: string): void {
    const existing = this.pending.get(noteId);

    if (existing) {
      clearTimeout(existing.timer);
    }

    const fromTitle = existing?.fromTitle ?? previousTitle;
    const timer = setTimeout(() => this.enqueue(noteId, fromTitle), RENAME_DEBOUNCE_MS);

    this.pending.set(noteId, { fromTitle, timer });
  }

  /**
   * Runs `operation` (a move of notes, a rename or move of folders), then points the links at the notes whose path it
   * changed: the note paths are compared before and after it.
   */
  async keepLinks<T>(operation: () => Promise<T>): Promise<T> {
    const before = this.notePaths();
    const result = await operation();
    const moves = new Map<string, string>();

    for (const [id, path] of this.notePaths()) {
      const previous = before.get(id);

      if (previous !== undefined && pathKey(previous) !== pathKey(path)) {
        moves.set(pathKey(previous), path);
      }
    }

    if (moves.size > 0) {
      this.chain(() => this.retarget(moves, null));
    }
    return result;
  }

  /** Path of every note that links can reach, by id. */
  private notePaths(): Map<number, string> {
    const folders = this.folders.folders();
    return new Map(this.notes.notes().filter(n => !n.tags.includes(QUICK_NOTE_TAG)).map(n => [n.id, notePath(n, folders)]));
  }

  private enqueue(noteId: number, fromTitle: string): void {
    this.pending.delete(noteId);
    this.chain(() => this.rewriteLinks(noteId, fromTitle));
  }

  private chain(rewrite: () => Promise<void>): void {
    // A failure must not break the chain; the links stay as they were
    this.queue = this.queue
      .then(rewrite)
      .catch(err => console.error('[LinkRename] Failed to update the links:', err));
  }

  private async rewriteLinks(noteId: number, fromTitle: string): Promise<void> {
    const renamed = this.notes.notes().find(n => n.id === noteId);

    if (!renamed || renamed.tags.includes(QUICK_NOTE_TAG)) {
      return;
    }

    const folders = this.folders.folders();
    const fromPath = notePath({ ...renamed, title: fromTitle }, folders);
    const toPath = notePath(renamed, folders);

    if (pathKey(fromPath) !== pathKey(toPath)) {
      // The retitled note is being typed in: its own content is left alone
      await this.retarget(new Map([[pathKey(fromPath), toPath]]), noteId);
    }
  }

  /** Rewrites the links of every note (but `skipId`) to the moved paths. */
  private async retarget(moves: ReadonlyMap<string, string>, skipId: number | null): Promise<void> {
    // Cheap filter: a note without any markdown link has nothing to rewrite
    const candidates = this.notes.notes().filter(n => n.id !== skipId && !n.tags.includes(QUICK_NOTE_TAG) && n.content.includes(']('));
    let sliceStart = performance.now();

    for (const candidate of candidates) {
      // Re-read: the note may have been edited since the list was taken
      const current = this.notes.notes().find(n => n.id === candidate.id);
      const content = current ? retargetNoteLinks(current.content, moves) : null;

      if (content !== null) {
        await this.notes.update(candidate.id, { content });
      }

      if (performance.now() - sliceStart > WORK_SLICE_MS) {
        await new Promise<void>(resolve => setTimeout(resolve));
        sliceStart = performance.now();
      }
    }
  }
}
