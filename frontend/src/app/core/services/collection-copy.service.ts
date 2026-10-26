import { Injectable, inject } from '@angular/core';
import {
  CollectionCopySource,
  CopiedNote,
  copiedNoteContent,
  copiedNotePaths,
  copiedNoteTitle,
  planCollectionCopy,
  retargetCopiedLinks,
} from '../utils/collection-copy.util';
import { CollectionsService } from './collections.service';
import { DesktopBridgeService } from './desktop-bridge.service';
import { FoldersService } from './folders.service';
import { NotesService } from './notes.service';

/** Outcome of a copy: the notes created, and the files left behind (unreadable, or their folder could not be made). */
export interface CollectionCopyResult {
  copied: number;
  failed: number;
}

/** "Copy To…" of the collections: markdown files of the disk become notes, their folders become note folders. */
@Injectable({ providedIn: 'root' })
export class CollectionCopyService {
  private readonly collections = inject(CollectionsService);
  private readonly bridge = inject(DesktopBridgeService);
  private readonly folders = inject(FoldersService);
  private readonly notes = inject(NotesService);

  /** Copies the sources into `targetFolderId` (`null`: the root of the notes); one failure never stops the others. */
  async copyToNotes(sources: readonly CollectionCopySource[], targetFolderId: number | null): Promise<CollectionCopyResult> {
    const plan = planCollectionCopy(sources);
    // Id of each planned folder once created, `undefined` when it (or one of its parents) could not be
    const folderIds: (number | undefined)[] = [];

    for (const folder of plan.folders) {
      const parentId = folder.parent === null ? targetFolderId : folderIds[folder.parent];
      const created = parentId === undefined ? null : await this.folders.create(folder.name, parentId).catch(() => null);
      folderIds.push(created?.id);
    }

    const result: CollectionCopyResult = { copied: 0, failed: 0 };
    const notes: CopiedNote[] = [];

    for (const file of plan.files) {
      const folderId = file.folder === null ? targetFolderId : folderIds[file.folder];

      if (folderId === undefined) {
        result.failed++;
      } else {
        notes.push({ ref: file.ref, title: copiedNoteTitle(file.name), folderId });
      }
    }

    // Known before any note is created, so the links between the copied files lead to their notes
    const paths = copiedNotePaths(notes, this.folders.folders());

    for (const note of notes) {
      try {
        const content = retargetCopiedLinks(await this.collections.read(note.ref), note.ref, paths);
        await this.bridge.createNote(note.title, copiedNoteContent(content, note.title), [], note.folderId);
        result.copied++;
      } catch {
        result.failed++;
      }
    }

    await this.notes.loadAll();
    return result;
  }
}
