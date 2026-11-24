import { ChangeDetectionStrategy, Component, computed, effect, inject, linkedSignal, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { FolderSummary } from '../../core/models/collection-sync.model';
import { CollectionSyncService } from '../../core/services/collection-sync.service';
import { acceptMode, formatBytes, needsMergeWarning } from '../../core/utils/collection-sync.util';

/**
 * A collection proposed by another device: where it goes on this one (the folder of the received collections, or
 * another), what to do when that folder already exists, then Accept / Ignore / Later. Nothing is written before Accept.
 */
@Component({
  selector: 'app-collection-offer-dialog',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'later()',
  },
  template: `
    @if (offer(); as current) {
      <div class="backdrop" (click)="later()"></div>
      <div class="dialog" role="dialog" aria-modal="true" [attr.aria-label]="title()">
        <h2 class="title">{{ title() }}</h2>
        <p class="message">
          {{ i18n.t('csync.offer.summary', { name: current.name, files: i18n.plural('csync.files', current.files), size: size() }) }}
        </p>

        <div class="field">
          {{ i18n.t('csync.offer.destination') }}
          <div class="destination">
            <input class="input path" type="text" readonly spellcheck="false" [value]="destination()" [title]="destination()" />
            <button class="btn secondary" type="button" [disabled]="busy()" (click)="pick()">{{ i18n.t('csync.offer.modify') }}</button>
          </div>
        </div>

        @if (warning() && folder(); as existing) {
          <div class="exists" role="group">
            <p class="message"><span aria-hidden="true">⚠</span> {{ i18n.plural('csync.offer.exists', existing.mdFiles) }}</p>
            <label class="radio">
              <input type="radio" name="existing" checked />
              <span>{{ i18n.t('csync.offer.merge') }}</span>
            </label>
            <p class="hint indented">{{ i18n.t('csync.offer.mergeHint') }}</p>
            <label class="radio">
              <input type="radio" name="existing" (click)="$event.preventDefault(); pick()" />
              <span>{{ i18n.t('csync.offer.otherFolder') }}</span>
            </label>
          </div>
        }

        <p class="hint">{{ i18n.t('csync.offer.nothingWritten') }}</p>
        @if (error(); as message) {
          <p class="error" role="alert">{{ message }}</p>
        }
        <div class="actions">
          <button class="btn" type="button" [disabled]="busy() || inspecting() || destination() === ''" (click)="accept()">{{ i18n.t('csync.offer.accept') }}</button>
          <button class="btn secondary" type="button" [disabled]="busy()" (click)="decline()">{{ i18n.t('csync.offer.decline') }}</button>
          <button class="btn secondary" type="button" [disabled]="busy()" (click)="later()">{{ i18n.t('csync.offer.later') }}</button>
        </div>
      </div>
    }
  `,
  styleUrls: ['../profiles/profile-dialog.scss'],
  styles: [`
    .destination { display: flex; gap: 6px; }
    .path { flex: 1; min-width: 0; text-overflow: ellipsis; }
    .exists { display: flex; flex-direction: column; gap: 4px; }
    .radio { display: flex; align-items: center; gap: 6px; font-size: 12.5px; color: var(--daftr-text); cursor: pointer; }
    .indented { margin-left: 22px; }
  `],
})
export class CollectionOfferDialogComponent {
  protected readonly i18n = inject(I18nService);
  private readonly collectionSync = inject(CollectionSyncService);

  protected readonly offer = this.collectionSync.offerInDialog;
  /** Starts from the suggested folder of the proposal, then follows "Change…". */
  protected readonly destination = linkedSignal(() => this.offer()?.suggestedDestination ?? '');
  protected readonly folder = signal<FolderSummary | null>(null);
  protected readonly inspecting = signal(false);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  /** Inspects the latest destination only. */
  private inspectToken = 0;

  protected readonly warning = computed(() => needsMergeWarning(this.folder()));
  protected readonly title = computed(() => this.i18n.t('csync.offer.title', { device: this.offer()?.device.name ?? '' }));
  protected readonly size = computed(() => formatBytes(this.offer()?.bytes ?? 0, this.i18n.language()));

  constructor() {
    effect(() => {
      const destination = this.destination();
      untracked(() => { void this.inspect(destination); });
    });
  }

  /** "Change…" or "Choose another folder": the native dialog; cancelled, nothing changes. */
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

  protected async accept(): Promise<void> {
    await this.answer(this.destination(), acceptMode(this.folder()));
  }

  protected async decline(): Promise<void> {
    await this.answer(null, 'decline');
  }

  /** "Later": the proposal stays in the list of the pending ones. */
  protected later(): void {
    this.collectionSync.closeOffer();
  }

  private async answer(destination: string | null, mode: 'receive' | 'merge' | 'decline'): Promise<void> {
    const offer = this.offer();

    if (!offer) {
      return;
    }

    this.busy.set(true);
    this.error.set(null);

    try {
      await this.collectionSync.answerOffer(offer, destination, mode);
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
