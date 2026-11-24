import { ChangeDetectionStrategy, Component, computed, effect, inject, input, linkedSignal, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { CollectionOffer, FolderSummary } from '../../core/models/collection-sync.model';
import { CollectionSyncService } from '../../core/services/collection-sync.service';
import { acceptMode, formatBytes, needsMergeWarning } from '../../core/utils/collection-sync.util';
import { SelectComponent, SelectOption } from '../../shared/ui/select.component';

/**
 * A collection received from another device and not set up yet, as a row of the sharing list in a lighter style (dashed
 * border, "To set up" badge). A click opens the form: profile, destination folder (with the warning when it already
 * holds notes), then Activate or Ignore. Nothing is written to the disk before Activate.
 */
@Component({
  selector: 'app-sync-offer-row',
  standalone: true,
  imports: [SelectComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="offer" [class.open]="open()">
      <button
        class="head"
        type="button"
        [attr.aria-expanded]="open()"
        [attr.aria-label]="i18n.t('csync.offer.expand', { name: offer().name })"
        (click)="open.set(!open())"
      >
        <svg class="chevron" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <polyline points="9 6 15 12 9 18"/>
        </svg>
        <span class="text">
          <span class="line">
            <span class="name">{{ offer().name }}</span>
            <span class="badge">{{ i18n.t('csync.offer.badge') }}</span>
          </span>
          <span class="meta">{{ meta() }}</span>
        </span>
      </button>

      @if (open()) {
        <div class="form">
          <div class="field">
            <span class="label">{{ i18n.t('csync.offer.profile') }}</span>
            <app-select [value]="profileId()" [options]="profileOptions()" [ariaLabel]="i18n.t('csync.offer.profile')" (valueChange)="profileId.set($event)" />
          </div>

          <div class="field">
            <span class="label">{{ i18n.t('csync.offer.destination') }}</span>
            <div class="destination">
              <input class="input path" type="text" readonly spellcheck="false" [value]="destination()" [title]="destination()" />
              <button class="btn secondary" type="button" [disabled]="busy()" (click)="pick()">{{ i18n.t('csync.offer.modify') }}</button>
            </div>
          </div>

          @if (warning() && folder(); as existing) {
            <p class="warn" role="group">
              <span aria-hidden="true">⚠</span> {{ i18n.plural('csync.offer.exists', existing.mdFiles) }}
              {{ i18n.t('csync.offer.mergeHint') }}
            </p>
          }

          <p class="hint">{{ i18n.t('csync.offer.nothingWritten') }}</p>
          @if (error(); as message) {
            <p class="error" role="alert">{{ message }}</p>
          }
          <div class="actions">
            <button class="btn" type="button" [disabled]="busy() || inspecting() || destination() === ''" (click)="activate()">{{ i18n.t('csync.offer.activate') }}</button>
            <button class="btn secondary" type="button" [disabled]="busy()" (click)="ignore()">{{ i18n.t('csync.offer.ignore') }}</button>
          </div>
        </div>
      }
    </div>
  `,
  styleUrls: ['./sync-screen-ui.scss'],
  styles: [`
    :host { display: block; }
    .offer { border: 1px dashed var(--daftr-border); border-radius: 6px; background: rgba(var(--daftr-hover-rgb), 0.03); }
    .offer.open { border-color: var(--daftr-accent); }
    .head {
      display: flex;
      align-items: center;
      gap: 8px;
      width: 100%;
      padding: 6px 10px;
      border: none;
      background: none;
      color: inherit;
      font: inherit;
      text-align: left;
      cursor: pointer;
    }
    .chevron { flex: none; color: var(--daftr-text-soft); transition: transform 0.1s; }
    .open .chevron { transform: rotate(90deg); }
    .text { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
    .line { display: flex; align-items: center; gap: 8px; min-width: 0; }
    .name { overflow: hidden; color: var(--daftr-text); text-overflow: ellipsis; white-space: nowrap; }
    .badge {
      flex: none;
      padding: 0 6px;
      border: 1px solid var(--daftr-accent);
      border-radius: 8px;
      color: var(--daftr-accent-text);
      font-size: 10.5px;
      line-height: 16px;
    }
    .meta { overflow: hidden; font-size: 11px; color: var(--daftr-text-dimmed); text-overflow: ellipsis; white-space: nowrap; }
    .form { display: flex; flex-direction: column; gap: 10px; padding: 4px 10px 10px 29px; }
    .field { display: flex; flex-direction: column; gap: 4px; }
    .destination { display: flex; gap: 6px; }
    .path { flex: 1; min-width: 0; text-overflow: ellipsis; }
    .warn { margin: 0; font-size: 12px; line-height: 1.5; color: var(--daftr-text); }
    .actions { display: flex; gap: 8px; }
    app-select { display: block; }
  `],
})
export class SyncOfferRowComponent {
  protected readonly i18n = inject(I18nService);
  private readonly collectionSync = inject(CollectionSyncService);

  readonly offer = input.required<CollectionOffer>();
  /** The local profiles the collection can go into. */
  readonly profileOptions = input.required<SelectOption[]>();
  /** The profile the form starts on. */
  readonly defaultProfile = input.required<string>();

  protected readonly open = signal(false);
  protected readonly profileId = linkedSignal(() => this.defaultProfile());
  /** Starts from the suggested folder of the entry, then follows "Change…". */
  protected readonly destination = linkedSignal(() => this.offer().suggestedDestination);
  protected readonly folder = signal<FolderSummary | null>(null);
  protected readonly inspecting = signal(false);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  /** Inspects the latest destination only. */
  private inspectToken = 0;

  protected readonly warning = computed(() => needsMergeWarning(this.folder()));
  protected readonly meta = computed(() => {
    const offer = this.offer();

    return this.i18n.t('csync.offer.meta', {
      device: offer.device.name,
      files: this.i18n.plural('csync.files', offer.files),
      size: formatBytes(offer.bytes, this.i18n.language()),
    });
  });

  constructor() {
    // The folder is only read once the form is open
    effect(() => {
      const destination = this.destination();

      if (this.open()) {
        untracked(() => { void this.inspect(destination); });
      }
    });
  }

  /** "Change…": the native dialog; cancelled, nothing changes. */
  protected async pick(): Promise<void> {
    try {
      const picked = await this.collectionSync.pickFolder();

      if (picked) {
        this.destination.set(picked);
      }
    } catch (error) {
      this.error.set(this.collectionSync.errorMessage(error));
    }
  }

  protected async activate(): Promise<void> {
    await this.run(() => this.collectionSync.activate(this.offer(), this.profileId(), this.destination(), acceptMode(this.folder())));
  }

  protected async ignore(): Promise<void> {
    await this.run(() => this.collectionSync.ignore(this.offer()));
  }

  private async run(action: () => Promise<void>): Promise<void> {
    this.busy.set(true);
    this.error.set(null);

    try {
      await action();
    } catch (error) {
      this.error.set(this.collectionSync.errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }

  private async inspect(destination: string): Promise<void> {
    const token = ++this.inspectToken;
    this.folder.set(null);
    this.error.set(null);

    if (destination === '') {
      this.inspecting.set(false);
      return;
    }

    this.inspecting.set(true);

    try {
      const folder = await this.collectionSync.inspectFolder(destination);

      if (token === this.inspectToken) {
        this.folder.set(folder);
      }
    } catch (error) {
      if (token === this.inspectToken) {
        this.error.set(this.collectionSync.errorMessage(error));
      }
    } finally {
      if (token === this.inspectToken) {
        this.inspecting.set(false);
      }
    }
  }
}
