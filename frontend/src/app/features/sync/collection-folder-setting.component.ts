import { ChangeDetectionStrategy, Component, computed, effect, inject, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { CollectionSyncService } from '../../core/services/collection-sync.service';
import { SettingsService } from '../../core/services/settings.service';

/** "Received collections folder" of the sync settings: its path, and "Browse…" (native folder dialog). */
@Component({
  selector: 'app-collection-folder-setting',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="row">
      <input class="field path" type="text" readonly spellcheck="false" [value]="path()" [title]="path()" />
      <button class="btn" type="button" [disabled]="busy()" (click)="browse()">{{ i18n.t('csync.settings.browse') }}</button>
    </div>
    @if (error(); as message) {
      <div class="error">{{ message }}</div>
    }
  `,
  styleUrls: ['./sync-panel.scss'],
  styles: [`
    .path { flex: 1; min-width: 200px; text-overflow: ellipsis; }
  `],
})
export class CollectionFolderSettingComponent {
  protected readonly i18n = inject(I18nService);
  private readonly settingsService = inject(SettingsService);
  private readonly collectionSync = inject(CollectionSyncService);

  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  /** The folder in effect: the setting, or the default one the main process resolves when it is empty. */
  private readonly configured = computed(() => this.settingsService.settings().receivedCollectionsFolder);
  protected readonly path = computed(() => this.configured() || this.collectionSync.receivedFolder());

  constructor() {
    // The resolved folder follows the setting, also when it is reset from the generic control of the editor
    effect(() => {
      this.configured();
      untracked(() => { void this.collectionSync.load(); });
    });
  }

  protected async browse(): Promise<void> {
    this.busy.set(true);
    this.error.set(null);

    try {
      const picked = await this.collectionSync.pickFolder();

      if (picked) {
        await this.settingsService.update({ receivedCollectionsFolder: picked });
      }
    } catch (error) {
      this.error.set(this.collectionSync.errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
