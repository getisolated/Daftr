import { ChangeDetectionStrategy, Component, OnDestroy, computed, effect, inject, input, output, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { CollectionOffer, CollectionShare } from '../../core/models/collection-sync.model';
import { ProfileSync, SpaceDevice, TrustedDevice } from '../../core/models/sync.model';
import { CollectionSyncService } from '../../core/services/collection-sync.service';
import { ProfilesService } from '../../core/services/profiles.service';
import { SyncService } from '../../core/services/sync.service';
import { offersOfProfile, shareBadge, sharesOfProfile } from '../../core/utils/collection-sync.util';
import {
  formatPairingCode, needsNewPairingCode, offerTargets, sharedDevices, shortDeviceId, svgDataUrl,
} from '../../core/utils/sync.util';
import { ConfirmDialogAction, ConfirmDialogComponent } from '../../shared/ui/confirm-dialog.component';
import { DeviceIconComponent } from './device-icon.component';
import { pairingCountdown } from './pairing-countdown';
import { SyncJoinFormComponent } from './sync-join-form.component';

/**
 * "Devices and sync" of a profile, a sub-view of its settings dialog: a pairing code (QR code and 6 digits) always
 * valid while shown, the devices the profile is synchronized with, and "Stop" for one of them. A profile is synchronized
 * as soon as it is shared with a device: there is no on/off. Every action applies at once.
 */
@Component({
  selector: 'app-profile-devices',
  standalone: true,
  imports: [ConfirmDialogComponent, DeviceIconComponent, SyncJoinFormComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'onEscape()',
  },
  templateUrl: './profile-devices.component.html',
  styleUrls: ['./profile-devices.component.scss'],
})
export class ProfileDevicesComponent implements OnDestroy {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly collectionSync = inject(CollectionSyncService);
  private readonly profiles = inject(ProfilesService);

  readonly profileId = input.required<string>();
  /** Back to the main view of the dialog (button or Escape). */
  readonly back = output<void>();

  private readonly profileSync = signal<ProfileSync | null>(null);
  /** "Enter the code of another device": the form replaces the code, whose pairing mode is closed meanwhile. */
  protected readonly joining = signal(false);
  private readonly starting = signal(false);
  protected readonly pairingError = signal<string | null>(null);
  protected readonly offering = signal(false);
  protected readonly stopping = signal<SpaceDevice | null>(null);
  protected readonly busy = signal(false);
  protected readonly notice = signal<string | null>(null);
  protected readonly error = signal<string | null>(null);

  /** The code of the pairing mode opened here, not one opened by the sync settings. */
  private readonly ticket = computed(() => (this.sync.pairingOwner() === 'profile' ? this.sync.pairing() : null));
  private readonly timer = pairingCountdown(this.ticket, () => !this.joining());
  protected readonly liveTicket = computed(() => (this.timer.remaining() > 0 ? this.ticket() : null));
  protected readonly countdown = this.timer.label;
  protected readonly renewed = this.sync.pairingRenewed;
  protected readonly qrUrl = computed(() => {
    const ticket = this.liveTicket();
    return ticket ? svgDataUrl(ticket.qrSvg) : '';
  });
  protected readonly shared = computed(() => sharedDevices(this.profileSync()));
  protected readonly targets = computed(() => offerTargets(this.sync.trustedDevices(), this.profileSync()));
  protected readonly stopActions = computed<ConfirmDialogAction[]>(() => [
    { id: 'stop', label: this.i18n.t('sync.profile.stop'), danger: true },
  ]);
  protected readonly stoppingShare = signal<CollectionShare | null>(null);
  protected readonly shares = computed(() => sharesOfProfile(this.collectionSync.shares(), this.profileId()));
  /** Proposals not answered yet, "Later" ones included. */
  protected readonly proposed = computed(() => offersOfProfile(this.collectionSync.offers(), this.profileId()));
  /** A collection is shared from the active profile only: the assistant reads its collections. */
  protected readonly canAddCollection = computed(() => this.profileId() === this.profiles.activeId());
  protected readonly stopShareActions = computed<ConfirmDialogAction[]>(() => [
    { id: 'stop', label: this.i18n.t('csync.profile.stop'), danger: true },
  ]);

  constructor() {
    void this.sync.loadDevices();

    // Members read again for another profile, and on every change of the trusted devices (linking, offer accepted,
    // presence): a device linked by the code shows up here
    effect(() => {
      const profileId = this.profileId();
      this.sync.devices();
      untracked(() => { void this.load(profileId); });
    });

    // Always a valid code on screen: a new one at opening, after a linking, and once the previous one expired
    effect(() => {
      const waiting = this.joining() || this.starting() || this.pairingError() !== null;

      if (needsNewPairingCode(this.ticket(), this.timer.remaining(), waiting)) {
        untracked(() => { void this.startPairing(); });
      }
    });

    effect(() => {
      if (this.joining()) {
        untracked(() => { void this.stopPairing(); });
      }
    });
  }

  ngOnDestroy(): void {
    void this.stopPairing();
  }

  protected formattedCode(code: string): string {
    return formatPairingCode(code);
  }

  protected nameOf(device: SpaceDevice): string {
    return device.name || this.idOf(device);
  }

  protected idOf(device: SpaceDevice): string {
    return shortDeviceId(device.fingerprint);
  }

  /** Escape closes the form first, then leaves the sub-view; with the confirmation open, only the confirmation closes. */
  protected onEscape(): void {
    if (this.stopping() || this.stoppingShare() || this.collectionSync.assistant() || this.collectionSync.offerInDialog()) {
      return;
    }

    if (this.joining()) {
      this.joining.set(false);
    }
    else {
      this.back.emit();
    }
  }

  protected async offer(device: TrustedDevice): Promise<void> {
    const offered = await this.run(() => this.sync.offerProfile(this.profileId(), device.key));

    if (offered) {
      this.offering.set(false);
      this.notice.set(this.i18n.t('sync.profile.offerSent', { name: device.name }));
    }
  }

  /** State of a shared collection, as its badge tooltip says it ("Waiting for PC", "Folder not found"…). */
  protected shareState(share: CollectionShare): string {
    return shareBadge(share, Date.now(), this.i18n.language()).tooltip;
  }

  protected shareDevices(share: CollectionShare): string {
    return share.devices.map(device => device.name).join(', ');
  }

  protected addCollection(): void {
    this.collectionSync.openAssistant(null, 'devices');
  }

  protected answerProposal(offer: CollectionOffer): void {
    this.collectionSync.openOffer(offer);
  }

  protected async relink(share: CollectionShare): Promise<void> {
    await this.run(() => this.collectionSync.relink(share.root));
  }

  protected async onStopShareChosen(share: CollectionShare, choice: string | null): Promise<void> {
    this.stoppingShare.set(null);

    if (choice === 'stop') {
      await this.run(() => this.collectionSync.stop(share));
    }
  }

  protected async onStopChosen(device: SpaceDevice, choice: string | null): Promise<void> {
    this.stopping.set(null);

    if (choice === 'stop') {
      await this.run(() => this.sync.stopSharing(this.profileId(), device.key));
    }
  }

  private async startPairing(): Promise<void> {
    this.starting.set(true);
    this.pairingError.set(null);

    try {
      await this.sync.startPairing([this.profileId()], 'profile');
    } catch (error) {
      this.pairingError.set(this.sync.errorMessage(error));
    } finally {
      this.starting.set(false);
    }
  }

  private async stopPairing(): Promise<void> {
    try {
      await this.sync.stopPairing('profile');
    } catch (error) {
      // The mode expires by itself after a few minutes: nothing to show for a sub-view being left
      console.error('[Sync] Pairing not stopped:', error);
    }
  }

  private async load(profileId: string): Promise<void> {
    try {
      const loaded = await this.sync.profileSync(profileId);

      if (profileId === this.profileId()) {
        this.profileSync.set(loaded);
      }
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
    }
  }

  /** Runs an action, then reads the members of the profile again. */
  private async run(action: () => Promise<void>): Promise<boolean> {
    this.busy.set(true);
    this.error.set(null);
    this.notice.set(null);

    try {
      await action();
      return true;
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
      return false;
    } finally {
      await this.load(this.profileId());
      this.busy.set(false);
    }
  }
}
