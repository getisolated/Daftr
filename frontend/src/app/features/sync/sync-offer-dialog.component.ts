import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { OfferDestination, OfferDestinationOption, OfferReceived, ProfileOffer } from '../../core/models/sync.model';
import { ProfilesService } from '../../core/services/profiles.service';
import { SyncService } from '../../core/services/sync.service';
import { profileDisplayName } from '../../core/utils/profile.util';
import { initialSyncLabel, offerDestinationOptions, parseOfferDestination } from '../../core/utils/sync.util';
import { SelectComponent } from '../../shared/ui/select.component';

/**
 * Profiles proposed by a device: where each one goes on this device (new profile, merge into a profile not synchronized
 * yet, or nothing), then the progress of their initial sync.
 */
@Component({
  selector: 'app-sync-offer-dialog',
  standalone: true,
  imports: [SelectComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'close()',
  },
  template: `
    <div class="backdrop" (click)="close()"></div>
    <div class="dialog" role="dialog" aria-modal="true" [attr.aria-label]="title()">
      <h2 class="title">{{ title() }}</h2>
      @if (!started()) {
        @if (offer(); as current) {
          <p class="message">{{ i18n.t('sync.offer.message') }}</p>
          @for (proposed of current.offers; track proposed.spaceId) {
            <div class="offer">
              <div class="offer-name">
                {{ proposed.name }}
                <span class="count">· {{ i18n.plural('sync.offer.notes', proposed.noteCount) }}</span>
              </div>
              <app-select
                [value]="destinationOf(proposed)"
                [options]="optionsOf(proposed)"
                [ariaLabel]="proposed.name"
                (valueChange)="choose(proposed, $event)"
              />
            </div>
          }
          <p class="hint">{{ i18n.t('sync.offer.mergeHint') }}</p>
        }
      } @else {
        @for (entry of progress(); track entry.profileId) {
          <p class="message">{{ entry.label }}</p>
        } @empty {
          <p class="message">{{ i18n.t('sync.offer.waiting') }}</p>
        }
        @if (allDone()) {
          <p class="message">{{ i18n.t('sync.offer.done') }}</p>
        }
      }
      @if (error(); as message) {
        <p class="error" role="alert">{{ message }}</p>
      }
      <div class="actions">
        @if (!started()) {
          <button class="btn" type="button" [disabled]="busy()" (click)="start()">{{ i18n.t('sync.offer.start') }}</button>
          <button class="btn secondary" type="button" (click)="close()">{{ i18n.t('sync.offer.later') }}</button>
        } @else {
          <button class="btn secondary" type="button" (click)="close()">{{ i18n.t('common.close') }}</button>
        }
      </div>
    </div>
  `,
  styleUrls: ['../profiles/profile-dialog.scss'],
  styles: [`
    .offer { display: flex; flex-direction: column; gap: 4px; }
    .offer-name { font-size: 12.5px; color: var(--daftr-text-strong); }
    .count { color: var(--daftr-text-dimmed); }
    app-select { display: block; }
  `],
})
export class SyncOfferDialogComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly profiles = inject(ProfilesService);

  /** The oldest proposal waiting for an answer. */
  protected readonly offer = computed<OfferReceived | null>(() => this.sync.offers()[0] ?? null);
  /** Device of the proposal answered, kept for the title while its profiles sync. */
  private readonly answeredDevice = signal<string | null>(null);
  protected readonly started = computed(() => this.answeredDevice() !== null);
  /** Choice per proposed space; a new profile until changed. */
  private readonly destinations = signal<ReadonlyMap<string, string>>(new Map());
  private readonly acceptedCount = signal(0);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected readonly title = computed(() =>
    this.i18n.t('sync.offer.title', { device: this.answeredDevice() ?? this.offer()?.device.name ?? '' }));

  protected readonly progress = computed(() => [...this.sync.initialSyncs()].map(([profileId, sync]) => {
    const profile = this.profiles.profiles().find(candidate => candidate.id === profileId);
    const name = profile ? profileDisplayName(profile, this.i18n.t) : profileId;
    const label = sync.done
      ? this.i18n.t('sync.offer.profileDone', { name })
      : `${name} · ${initialSyncLabel(sync.progress, this.i18n.language())}`;

    return { profileId, label };
  }));

  protected readonly allDone = computed(() => {
    const done = [...this.sync.initialSyncs().values()].filter(sync => sync.done).length;
    return this.started() && done >= this.acceptedCount();
  });

  protected optionsOf(proposed: ProfileOffer): OfferDestinationOption[] {
    return offerDestinationOptions(
      proposed,
      this.profiles.profiles(),
      this.sync.syncedProfileIds(),
      profile => profileDisplayName(profile, this.i18n.t),
      this.i18n.language(),
    );
  }

  protected destinationOf(proposed: ProfileOffer): string {
    return this.destinations().get(proposed.spaceId) ?? 'new';
  }

  protected choose(proposed: ProfileOffer, value: string): void {
    this.destinations.update(destinations => new Map(destinations).set(proposed.spaceId, value));
  }

  protected async start(): Promise<void> {
    const offer = this.offer();

    if (!offer) {
      return;
    }

    const answers = new Map<string, OfferDestination>(
      offer.offers.map(proposed => [proposed.spaceId, parseOfferDestination(this.destinationOf(proposed))]),
    );
    this.busy.set(true);
    this.error.set(null);
    this.sync.clearInitialSyncs();

    try {
      await this.sync.answerOffer(offer, answers);
      this.acceptedCount.set([...answers.values()].filter(answer => answer.choice !== 'ignore').length);
      this.answeredDevice.set(offer.device.name);

      // Everything ignored: nothing to follow
      if (this.acceptedCount() === 0) {
        this.close();
      }
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }

  protected close(): void {
    this.sync.closeOfferDialog();
    this.sync.clearInitialSyncs();
  }
}
