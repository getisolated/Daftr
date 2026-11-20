import { Injectable, inject, signal } from '@angular/core';
import { I18nService } from '../i18n/i18n.service';
import { TransferItems } from '../models/profile.model';
import { CollectionsService } from './collections.service';
import { FileDocsService } from './file-docs.service';
import { FoldersService } from './folders.service';
import { NotesService } from './notes.service';
import { PendingSavesService } from './pending-saves.service';
import { ProfilesBridgeService } from './profiles-bridge.service';
import { SidebarStateService } from './sidebar-state.service';
import { TabsService } from './tabs.service';

/**
 * "Transfer to profile…": moves notes, folders and collections to another profile, then forgets them here (lists
 * reloaded, their tabs closed). The target profile is written to without being unlocked.
 */
@Injectable({ providedIn: 'root' })
export class ProfileTransferService {
  private readonly bridge = inject(ProfilesBridgeService);
  private readonly pendingSaves = inject(PendingSavesService);
  private readonly notes = inject(NotesService);
  private readonly folders = inject(FoldersService);
  private readonly collections = inject(CollectionsService);
  private readonly fileDocs = inject(FileDocsService);
  private readonly tabs = inject(TabsService);
  private readonly sidebarState = inject(SidebarStateService);
  private readonly i18n = inject(I18nService);

  private readonly _failure = signal<string | null>(null);
  /** Message of the last failed transfer, shown until dismissed. */
  readonly failure = this._failure.asReadonly();

  dismissFailure(): void {
    this._failure.set(null);
  }

  async transfer(targetId: string, items: TransferItems): Promise<void> {
    // The copy reads the notes from the database: an edit still waiting for its debounce goes with them
    await this.pendingSaves.flushAll();

    try {
      const moved = await this.bridge.transfer(targetId, items);
      const fileIds = moved.collections.flatMap(root => this.fileDocs.idsUnder(root));

      this.tabs.dropNotes(new Set([...moved.noteIds, ...fileIds]));

      if (moved.collections.includes(this.collections.selectedRoot() ?? '')) {
        this.collections.select(null);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this._failure.set(this.i18n.t('profiles.transfer.failed', { message }));
    } finally {
      await this.refresh();
    }
  }

  private async refresh(): Promise<void> {
    await this.folders.loadAll();
    await this.notes.loadAll();
    await this.collections.reload();
    this.sidebarState.prune(new Set(this.folders.folders().map(folder => folder.id)));
  }
}
