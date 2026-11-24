import { ChangeDetectionStrategy, Component, computed, effect, inject, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { FolderSummary } from '../../core/models/collection-sync.model';
import { SpaceDevice } from '../../core/models/sync.model';
import { CollectionSyncService } from '../../core/services/collection-sync.service';
import { CollectionsService } from '../../core/services/collections.service';
import { ProfilesService } from '../../core/services/profiles.service';
import { SyncService } from '../../core/services/sync.service';
import { ToastService } from '../../core/services/toast.service';
import { formatBytes, shareableCollections, tooLargeCount } from '../../core/utils/collection-sync.util';
import { sharedDevices } from '../../core/utils/sync.util';
import { CheckboxComponent } from '../../shared/ui/checkbox.component';
import { SelectComponent } from '../../shared/ui/select.component';

/**
 * "Sync with my devices…": which devices of the active profile receive a collection, with what the folder holds (files
 * and size, what is left out, a warning for a git folder). Opened for a collection of the sidebar, or from the profile
 * view where the collection is picked first.
 */
@Component({
  selector: 'app-collection-share-dialog',
  standalone: true,
  imports: [CheckboxComponent, SelectComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'close()',
  },
  template: `
    <div class="backdrop" (click)="close()"></div>
    <div class="dialog" role="dialog" aria-modal="true" [attr.aria-label]="title()">
      <h2 class="title">{{ title() }}</h2>

      @if (askCollection()) {
        @if (collectionOptions().length > 0) {
          <div class="field">
            {{ i18n.t('csync.share.collection') }}
            <app-select [value]="root() ?? ''" [options]="collectionOptions()" [ariaLabel]="i18n.t('csync.share.collection')" (valueChange)="root.set($event)" />
          </div>
        } @else {
          <p class="message">{{ i18n.t('csync.share.noCollection') }}</p>
        }
      }

      @if (root() !== null) {
        @if (loadingDevices()) {
          <p class="hint">{{ i18n.t('csync.share.summaryLoading') }}</p>
        } @else if (devices().length === 0) {
          <p class="message">{{ i18n.t('csync.share.noDevice') }}</p>
          <div class="actions start">
            <button class="btn secondary" type="button" (click)="linkDevice()">{{ i18n.t('csync.share.linkDevice') }}</button>
          </div>
        } @else {
          <div class="devices" role="group" [attr.aria-label]="i18n.t('csync.share.devices')">
            <div class="label">{{ i18n.t('csync.share.devices') }}</div>
            @for (device of devices(); track device.key) {
              <app-checkbox [checked]="selected().has(device.key)" (checkedChange)="toggle(device.key, $event)">
                {{ device.name || device.key }}
              </app-checkbox>
            }
          </div>
        }

        @if (summary(); as folder) {
          <p class="info">
            <span aria-hidden="true">ⓘ</span>
            {{ i18n.plural('csync.share.summary', folder.mdFiles, { size: size(folder) }) }}
          </p>
          @if (tooLarge(folder) > 0) {
            <p class="warn">{{ i18n.plural('csync.share.tooLarge', tooLarge(folder)) }}</p>
          }
          @if (otherIgnored(folder) > 0) {
            <p class="warn">{{ i18n.plural('csync.share.ignoredOther', otherIgnored(folder)) }}</p>
          }
          @if (folder.truncated) {
            <p class="warn">{{ i18n.t('csync.share.truncated') }}</p>
          }
          @if (folder.hasGit) {
            <p class="warn"><span aria-hidden="true">⚠</span> {{ i18n.t('csync.share.git') }}</p>
          }
        } @else if (summaryLoading()) {
          <p class="hint">{{ i18n.t('csync.share.summaryLoading') }}</p>
        }
      }

      @if (error(); as message) {
        <p class="error" role="alert">{{ message }}</p>
      }
      <div class="actions">
        <button class="btn" type="button" [disabled]="!canShare()" (click)="share()">{{ i18n.t('csync.share.confirm') }}</button>
        <button class="btn secondary" type="button" (click)="close()">{{ i18n.t('common.cancel') }}</button>
      </div>
    </div>
  `,
  styleUrls: ['../profiles/profile-dialog.scss'],
  styles: [`
    .devices { display: flex; flex-direction: column; gap: 6px; }
    .label { font-size: 12.5px; color: var(--daftr-text-strong); }
    .info, .warn { margin: 0; font-size: 12px; line-height: 1.5; color: var(--daftr-text-soft); }
    .warn { color: var(--daftr-text); }
    .actions.start { justify-content: flex-start; }
    app-select { display: block; }
  `],
})
export class CollectionShareDialogComponent {
  protected readonly i18n = inject(I18nService);
  private readonly collectionSync = inject(CollectionSyncService);
  private readonly collections = inject(CollectionsService);
  private readonly profiles = inject(ProfilesService);
  private readonly sync = inject(SyncService);
  private readonly toasts = inject(ToastService);

  private readonly request = this.collectionSync.assistant;
  /** The collection to share; a choice of the dialog when it was not given. */
  protected readonly root = signal<string | null>(untracked(this.request)?.root ?? null);
  protected readonly askCollection = computed(() => this.request()?.root === null);
  protected readonly devices = signal<SpaceDevice[]>([]);
  protected readonly loadingDevices = signal(true);
  protected readonly selected = signal<ReadonlySet<string>>(new Set());
  protected readonly summary = signal<FolderSummary | null>(null);
  protected readonly summaryLoading = signal(false);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  /** Reads the summary of the latest collection asked for only. */
  private summaryToken = 0;

  protected readonly collectionOptions = computed(() => {
    const shareable = new Set(shareableCollections(this.collections.states().map(state => state.collection), this.collectionSync.shares()).map(collection => collection.path));

    return this.collections.states()
      .filter(state => shareable.has(state.collection.path))
      .map(state => ({ value: state.collection.path, label: state.label }));
  });

  protected readonly name = computed(() => {
    const state = this.collections.states().find(candidate => candidate.collection.path === this.root());

    return state?.label ?? '';
  });

  protected readonly title = computed(() =>
    this.askCollection() ? this.i18n.t('csync.share.titlePick') : this.i18n.t('csync.share.title', { name: this.name() }));

  protected readonly canShare = computed(() => this.root() !== null && this.selected().size > 0 && !this.busy());

  constructor() {
    void this.loadDevices();

    // The first collection of the list is pre-selected when none was given
    effect(() => {
      const options = this.collectionOptions();

      if (this.askCollection() && this.root() === null && options[0]) {
        untracked(() => this.root.set(options[0]?.value ?? null));
      }
    });

    effect(() => {
      const root = this.root();
      untracked(() => { void this.loadSummary(root); });
    });
  }

  protected size(folder: FolderSummary): string {
    return formatBytes(folder.bytes, this.i18n.language());
  }

  protected tooLarge(folder: FolderSummary): number {
    return tooLargeCount(folder);
  }

  protected otherIgnored(folder: FolderSummary): number {
    return folder.ignored.length - tooLargeCount(folder);
  }

  protected toggle(key: string, checked: boolean): void {
    this.selected.update(selected => {
      const next = new Set(selected);

      if (checked) {
        next.add(key);
      } else {
        next.delete(key);
      }

      return next;
    });
  }

  /** The hint's button: the pairing view of the profile (already on screen when the assistant opened from there). */
  protected linkDevice(): void {
    const from = this.request()?.from;
    this.collectionSync.closeAssistant();

    if (from === 'sidebar') {
      this.profiles.requestDevices();
    }
  }

  protected async share(): Promise<void> {
    const root = this.root();

    if (root === null) {
      return;
    }

    this.busy.set(true);
    this.error.set(null);

    try {
      const share = await this.collectionSync.share(root, [...this.selected()]);
      const names = this.devices().filter(device => this.selected().has(device.key)).map(device => device.name || device.key).join(', ');
      this.toasts.show(this.i18n.t('csync.share.sent', { name: share.name, devices: names }));
      this.collectionSync.closeAssistant();
    } catch (error) {
      this.error.set(this.collectionSync.errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }

  protected close(): void {
    this.collectionSync.closeAssistant();
  }

  /** The devices that already share the active profile: the only ones a collection can be shared with. */
  private async loadDevices(): Promise<void> {
    try {
      const profile = await this.sync.profileSync(this.profiles.activeId());
      const devices = sharedDevices(profile);
      this.devices.set(devices);
      // All checked: the usual case is "my other computer"
      this.selected.set(new Set(devices.map(device => device.key)));
    } catch (error) {
      this.error.set(this.collectionSync.errorMessage(error));
    } finally {
      this.loadingDevices.set(false);
    }
  }

  private async loadSummary(root: string | null): Promise<void> {
    const token = ++this.summaryToken;
    this.summary.set(null);

    if (root === null) {
      this.summaryLoading.set(false);
      return;
    }

    this.summaryLoading.set(true);

    try {
      const summary = await this.collectionSync.summary(root);

      if (token === this.summaryToken) {
        this.summary.set(summary);
      }
    } catch (error) {
      if (token === this.summaryToken) {
        this.error.set(this.collectionSync.errorMessage(error));
      }
    } finally {
      if (token === this.summaryToken) {
        this.summaryLoading.set(false);
      }
    }
  }
}
