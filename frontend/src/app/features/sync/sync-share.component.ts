import { NgTemplateOutlet } from '@angular/common';
import { ChangeDetectionStrategy, Component, computed, effect, inject, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { CollectionOffer, FolderSummary } from '../../core/models/collection-sync.model';
import { CollectionSyncService } from '../../core/services/collection-sync.service';
import { ProfilesService } from '../../core/services/profiles.service';
import { SyncService } from '../../core/services/sync.service';
import { formatBytes, tooLargeCount } from '../../core/utils/collection-sync.util';
import { profileDisplayName } from '../../core/utils/profile.util';
import {
  MatrixRow, collectionRows, defaultOfferProfile, groupOffers, matrixDevices, profileRow,
} from '../../core/utils/sync-share.util';
import { CheckboxComponent } from '../../shared/ui/checkbox.component';
import { SelectComponent, SelectOption } from '../../shared/ui/select.component';
import { SyncOfferRowComponent } from './sync-offer-row.component';

/**
 * "Sharing" of the sync screen: a profile selector and a matrix, one row for the profile itself then one per folder
 * collection, one column of boxes per trusted device. The entries "to configure" are rows of the same list (a lighter
 * style, a form that opens on a click); the ones with no local profile come last, whichever profile is selected. A
 * ticked box applies at once.
 */
@Component({
  selector: 'app-sync-share',
  standalone: true,
  imports: [NgTemplateOutlet, CheckboxComponent, SelectComponent, SyncOfferRowComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './sync-share.component.html',
  styleUrls: ['./sync-screen-ui.scss', './sync-share.component.scss'],
})
export class SyncShareComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly collectionSync = inject(CollectionSyncService);
  private readonly profiles = inject(ProfilesService);

  /** The profile picked in the selector; the active one until a pick. */
  private readonly picked = signal<string | null>(null);
  /** Boxes being applied (`row id` + `device key`): a second click waits. */
  protected readonly applying = signal<ReadonlySet<string>>(new Set());
  /** What a collection holds, shown once under its row when it starts to be shared. */
  protected readonly notices = signal<ReadonlyMap<string, FolderSummary>>(new Map());
  protected readonly error = signal<string | null>(null);

  protected readonly profileOptions = computed<SelectOption[]>(() =>
    this.profiles.profiles().map(profile => ({ value: profile.id, label: profileDisplayName(profile, this.i18n.t) })));
  protected readonly profileId = computed(() => {
    const picked = this.picked();
    return picked !== null && this.profiles.profiles().some(profile => profile.id === picked) ? picked : this.profiles.activeId();
  });
  protected readonly profile = computed(() => this.profiles.profiles().find(candidate => candidate.id === this.profileId()) ?? null);
  protected readonly devices = computed(() => matrixDevices(this.sync.devices()));
  /** One narrow column per device after the label. */
  protected readonly columns = computed(() => `minmax(0, 1fr) repeat(${this.devices().length}, 96px)`);
  protected readonly profileLine = computed<MatrixRow>(() => {
    const profile = this.profile();
    const label = profile ? profileDisplayName(profile, this.i18n.t) : '';

    return profileRow(this.profileId(), this.i18n.t('sync.screen.profileRow', { name: label }), this.sync.profileShares(), this.devices());
  });
  protected readonly rows = computed(() =>
    collectionRows(this.profileId(), this.collectionSync.profileCollections(), this.collectionSync.shares(), this.devices()));
  private readonly knownProfileIds = computed(() => this.profiles.profiles().map(profile => profile.id));
  private readonly groups = computed(() => groupOffers(this.collectionSync.offers(), this.profileId(), this.knownProfileIds()));
  protected readonly offersHere = computed(() => this.groups().forProfile);
  protected readonly offersWithoutProfile = computed(() => this.groups().withoutProfile);

  constructor() {
    void this.sync.loadDevices();
    void this.collectionSync.load();

    // The matrix follows the trusted devices (linking, removal, presence)
    effect(() => {
      this.sync.devices();
      untracked(() => { void this.sync.loadProfileShares(); });
    });
  }

  protected onProfilePicked(id: string): void {
    this.picked.set(id);
  }

  protected defaultProfileOf(offer: CollectionOffer): string {
    return defaultOfferProfile(offer, this.knownProfileIds(), this.profiles.activeId());
  }

  protected deviceName(key: string): string {
    return this.devices().find(device => device.key === key)?.name ?? key;
  }

  protected isApplying(row: MatrixRow, deviceKey: string): boolean {
    return this.applying().has(`${row.id}|${deviceKey}`);
  }

  protected noticeOf(row: MatrixRow): FolderSummary | null {
    return this.notices().get(row.id) ?? null;
  }

  protected size(folder: FolderSummary): string {
    return formatBytes(folder.bytes, this.i18n.language());
  }

  protected tooLarge(folder: FolderSummary): number {
    return tooLargeCount(folder);
  }

  protected dismissNotice(row: MatrixRow): void {
    this.notices.update(notices => {
      const next = new Map(notices);
      next.delete(row.id);
      return next;
    });
  }

  protected async toggle(row: MatrixRow, deviceKey: string, checked: boolean): Promise<void> {
    const cell = `${row.id}|${deviceKey}`;

    if (this.applying().has(cell)) {
      return;
    }

    this.applying.update(cells => new Set(cells).add(cell));
    this.error.set(null);

    try {
      if (row.kind === 'profile') {
        await this.sync.setProfileShared(this.profileId(), deviceKey, checked);
      } else if (row.root !== null) {
        const first = checked && row.cells.every(candidate => !candidate.checked);
        await this.collectionSync.setDevice(this.profileId(), row.root, deviceKey, checked);

        if (first) {
          void this.showSummary(row, row.root);
        } else if (!checked) {
          this.dropNoticeWhenAlone(row, deviceKey);
        }
      }
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
    } finally {
      this.applying.update(cells => {
        const next = new Set(cells);
        next.delete(cell);
        return next;
      });
    }
  }

  /** What the collection shares, told once when it starts: the number of files, and a warning for a git folder. */
  private async showSummary(row: MatrixRow, root: string): Promise<void> {
    try {
      const summary = await this.collectionSync.summary(root, this.profileId());
      this.notices.update(notices => new Map(notices).set(row.id, summary));
    } catch (error) {
      console.error('[Sync] Summary not read:', error);
    }
  }

  /** The last device unchecked ends the sharing: its notice goes with it. */
  private dropNoticeWhenAlone(row: MatrixRow, deviceKey: string): void {
    if (row.cells.every(candidate => !candidate.checked || candidate.deviceKey === deviceKey)) {
      this.dismissNotice(row);
    }
  }
}
