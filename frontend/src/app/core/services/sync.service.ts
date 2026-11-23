import { Injectable, OnDestroy, computed, effect, inject, signal, untracked } from '@angular/core';
import { I18nService } from '../i18n/i18n.service';
import {
  InitialSyncDone, LinkedDevice, NotesSynced, OfferDestination, OfferReceived, PairingOwner, PairingTicket, ProfileSync, SyncOverview,
  SyncProgress, SyncStatus, TrustedDevice,
} from '../models/sync.model';
import { profileDisplayName } from '../utils/profile.util';
import { syncErrorMessage, syncIndicator } from '../utils/sync.util';
import { ProfilesService } from './profiles.service';
import { SettingsService } from './settings.service';
import { SyncBridgeService } from './sync-bridge.service';
import { ToastService } from './toast.service';

/** Category of the sync in the settings editor (see SETTING_DEFINITIONS). */
export const SYNC_SETTINGS_CATEGORY = 'sync.settings.category';

/** After "Link to a device", the profiles proposed by the new device open their choice at once for this long. */
const OFFER_EXPECTED_MS = 60_000;
/** `sync_pair` answers and `sync:device-linked` arrives for the same linking: a single notification. */
const LINK_NOTICE_MS = 5000;

/**
 * State of the LAN sync for the interface: status of the indicator, overview and trusted devices of the settings,
 * pairing mode, proposed profiles waiting for an answer, progress of the initial syncs, in-app notifications. The
 * engine runs in the main process; this service only follows its events and calls its commands.
 */
@Injectable({ providedIn: 'root' })
export class SyncService implements OnDestroy {
  private readonly bridge = inject(SyncBridgeService);
  private readonly i18n = inject(I18nService);
  private readonly profiles = inject(ProfilesService);
  private readonly settings = inject(SettingsService);
  private readonly toasts = inject(ToastService);

  private readonly _overview = signal<SyncOverview | null>(null);
  private readonly _status = signal<SyncStatus | null>(null);
  private readonly _devices = signal<TrustedDevice[]>([]);
  private readonly _pairing = signal<PairingTicket | null>(null);
  /** Screen that opened the pairing mode last; it stays set when the mode closes by itself. */
  private readonly _pairingOwner = signal<PairingOwner | null>(null);
  /** The pairing code was renewed after too many wrong attempts. */
  private readonly _pairingRenewed = signal(false);
  private readonly _offers = signal<OfferReceived[]>([]);
  private readonly _offerDialogOpen = signal(false);
  /** Initial sync of each profile that accepted an offer: progress, then `done`. */
  private readonly _initialSyncs = signal<ReadonlyMap<string, { progress: SyncProgress | null; done: boolean }>>(new Map());
  private readonly notesSyncedListeners = new Set<(event: NotesSynced) => void>();
  private readonly unsubscribers: (() => void)[];
  /** A pushed status is newer than the one of a pending `sync_state`. */
  private statusPushed = false;
  private devicesLoaded = false;
  private offersExpectedUntil = 0;
  private readonly lastLinkNotice = new Map<string, number>();
  /** The device settings as one value: unchanged while the other settings change. */
  private readonly deviceSettings = computed(() => {
    const { syncDeviceName, syncPort, syncPaused } = this.settings.settings();
    return JSON.stringify([syncDeviceName, syncPort, syncPaused]);
  });

  /** `null` until known, and outside the desktop app. */
  readonly overview = this._overview.asReadonly();
  readonly status = computed<SyncStatus | null>(() => this._status() ?? this._overview()?.status ?? null);
  readonly indicator = computed(() => syncIndicator(this.status(), this.i18n.language()));
  readonly devices = this._devices.asReadonly();
  /** Trusted devices not removed. */
  readonly trustedDevices = computed(() => this._devices().filter(device => device.revokedAt === null));
  readonly pairing = this._pairing.asReadonly();
  readonly pairingOwner = this._pairingOwner.asReadonly();
  readonly pairingRenewed = this._pairingRenewed.asReadonly();
  /** Proposals waiting for an answer, oldest first. */
  readonly offers = this._offers.asReadonly();
  readonly offerDialogOpen = this._offerDialogOpen.asReadonly();
  readonly initialSyncs = this._initialSyncs.asReadonly();
  /** Profiles of this device that have a space, synchronized or not. */
  readonly syncedProfileIds = computed(() => (this._overview()?.profiles ?? []).map(profile => profile.profileId));

  constructor() {
    this.unsubscribers = [
      this.bridge.onStatus(status => this.onStatus(status)),
      this.bridge.onNotesSynced(event => this.notesSyncedListeners.forEach(listener => listener(event))),
      this.bridge.onProgress(progress => this.onProgress(progress)),
      this.bridge.onInitialDone(event => void this.onInitialDone(event)),
      this.bridge.onDeviceLinked(device => void this.onDeviceLinked(device)),
      this.bridge.onOfferReceived(offer => void this.onOfferReceived(offer)),
      this.bridge.onPairing(state => this.onPairingTicket(state.active ? state.ticket : null)),
    ];
    // Read at start, and again when a device setting changes from the generic controls of the settings editor (port,
    // pause, reset of the name): the overview holds the effective name and the listening port
    effect(() => {
      this.deviceSettings();
      untracked(() => { void this.refresh(); });
    });
  }

  ngOnDestroy(): void {
    this.unsubscribers.forEach(unsubscribe => unsubscribe());
  }

  /** Message of a failed command, in the language of the interface. */
  errorMessage(error: unknown): string {
    return syncErrorMessage(error, this.i18n.language());
  }

  /** Follows the notes changed by another device (the editor applies them to the open note). */
  onNotesSynced(listener: (event: NotesSynced) => void): () => void {
    this.notesSyncedListeners.add(listener);
    return () => this.notesSyncedListeners.delete(listener);
  }

  /** Reads the overview of the settings (and the status, unless a newer one was pushed meanwhile). */
  async refresh(): Promise<void> {
    try {
      const overview = await this.bridge.overview();

      if (overview) {
        this._overview.set(overview);
        if (!this.statusPushed) this._status.set(overview.status);
      }
    } catch (error) {
      console.error('[Sync] State not read:', error);
    }
  }

  /** Reads the trusted devices; once read, they are read again on every change of the sync. */
  async loadDevices(): Promise<void> {
    this.devicesLoaded = true;

    try {
      this._devices.set(await this.bridge.devices());
    } catch (error) {
      console.error('[Sync] Devices not read:', error);
    }
  }

  async profileSync(profileId: string): Promise<ProfileSync | null> {
    return this.bridge.profile(profileId);
  }

  async syncNow(): Promise<void> {
    await this.bridge.syncNow();
  }

  async setDeviceName(name: string): Promise<void> {
    this._overview.set(await this.bridge.setDeviceName(name.trim()));
    await this.settings.load();
  }

  /**
   * "Link a device": opens the pairing mode for these profiles, on behalf of `owner`. It replaces a mode opened by
   * another screen, which then stops showing its code.
   */
  async startPairing(profileIds: string[], owner: PairingOwner): Promise<void> {
    this._pairingOwner.set(owner);
    const ticket = await this.bridge.startPairing(profileIds);

    // Stopped while opening (the screen closed): the mode must not stay open with its code shown nowhere
    if (this._pairingOwner() !== owner) {
      if (this._pairingOwner() === null) {
        await this.bridge.stopPairing();
      }

      return;
    }

    this._pairingRenewed.set(false);
    this._pairing.set(ticket);
    // The checked profiles got their space
    await this.refresh();
  }

  /** Closes the pairing mode, unless another screen than `owner` opened it since. */
  async stopPairing(owner: PairingOwner): Promise<void> {
    if (this._pairingOwner() !== owner) {
      return;
    }

    this._pairingOwner.set(null);
    this._pairing.set(null);
    await this.bridge.stopPairing();
  }

  /** "Link to a device": the profiles it then proposes open their choice at once. */
  async pair(code: string, address: string | null): Promise<void> {
    this.offersExpectedUntil = Date.now() + OFFER_EXPECTED_MS;

    try {
      const device = await this.bridge.pair(code, address);
      this.noticeLinked(device);
      await Promise.all([this.refresh(), this.reloadDevices()]);
    } catch (error) {
      this.offersExpectedUntil = 0;
      throw error;
    }
  }

  openOfferDialog(): void {
    if (this._offers().length > 0) {
      this._offerDialogOpen.set(true);
    }
  }

  /** Closes the choice; the proposals not answered stay for later (until the app quits). */
  closeOfferDialog(): void {
    this._offerDialogOpen.set(false);
  }

  /**
   * Answers every profile proposed by a device. The accepted ones then sync at once: their progress shows in
   * `initialSyncs`. A profile created by the answer appears in the profile list.
   */
  async answerOffer(offer: OfferReceived, destinations: ReadonlyMap<string, OfferDestination>): Promise<void> {
    for (const proposed of offer.offers) {
      const destination = destinations.get(proposed.spaceId) ?? { choice: 'ignore', profileId: null };
      await this.bridge.answerOffer(offer.device.key, proposed.spaceId, destination.choice, destination.profileId);
    }

    this._offers.update(offers => offers.filter(candidate => candidate !== offer));
    await Promise.all([this.profiles.load(), this.refresh()]);
  }

  async offerProfile(profileId: string, deviceKey: string): Promise<void> {
    await this.bridge.offerProfile(profileId, deviceKey);
    await this.refresh();
  }

  async stopSharing(profileId: string, deviceKey: string): Promise<void> {
    await this.bridge.stopSharing(profileId, deviceKey);
    await this.reloadDevices();
  }

  async removeDevice(deviceKey: string): Promise<void> {
    await this.bridge.removeDevice(deviceKey);
    await this.reloadDevices();
  }

  /** Forgets the initial syncs shown by the destination dialog once it closes. */
  clearInitialSyncs(): void {
    this._initialSyncs.set(new Map());
  }

  private onStatus(status: SyncStatus): void {
    this.statusPushed = true;
    this._status.set(status);
    // Presence and connections of the devices change with the status (no event of their own)
    void this.reloadDevices();
  }

  private onProgress(progress: SyncProgress): void {
    this._initialSyncs.update(syncs => new Map(syncs).set(progress.profileId, { progress, done: false }));
  }

  private async onInitialDone(event: InitialSyncDone): Promise<void> {
    const progress = this._initialSyncs().get(event.profileId)?.progress ?? null;
    this._initialSyncs.update(syncs => new Map(syncs).set(event.profileId, { progress, done: true }));
    // A profile created by an offer is not announced otherwise
    await this.profiles.load();
    const profile = this.profiles.profiles().find(candidate => candidate.id === event.profileId);
    const name = profile ? profileDisplayName(profile, this.i18n.t) : event.profileId;
    this.toasts.show(this.i18n.t('sync.toast.initialDone', { profile: name }));
    await Promise.all([this.refresh(), this.reloadDevices()]);
  }

  private async onDeviceLinked(device: LinkedDevice): Promise<void> {
    this.noticeLinked(device);
    await Promise.all([this.refresh(), this.reloadDevices()]);
  }

  private async onOfferReceived(offer: OfferReceived): Promise<void> {
    // A device proposing again replaces its previous proposal
    this._offers.update(offers => [...offers.filter(candidate => candidate.device.key !== offer.device.key), offer]);

    if (Date.now() < this.offersExpectedUntil) {
      this.offersExpectedUntil = 0;
      this._offerDialogOpen.set(true);
    }
    else {
      const profiles = offer.offers.map(proposed => proposed.name).join(', ');
      this.toasts.show(this.i18n.t('sync.toast.offer', { device: offer.device.name, profiles }), {
        label: this.i18n.t('sync.toast.choose'),
        run: () => this.openOfferDialog(),
      });
    }

    await this.profiles.load();
  }

  private onPairingTicket(ticket: PairingTicket | null): void {
    const current = this._pairing();
    this._pairingRenewed.set(ticket !== null && current !== null && ticket.code !== current.code);
    this._pairing.set(ticket);
  }

  /** "X is now linked", once per linking. */
  private noticeLinked(device: LinkedDevice): void {
    const now = Date.now();

    if (now - (this.lastLinkNotice.get(device.key) ?? 0) > LINK_NOTICE_MS) {
      this.toasts.show(this.i18n.t('sync.toast.linked', { name: device.name }));
    }

    this.lastLinkNotice.set(device.key, now);
  }

  private async reloadDevices(): Promise<void> {
    if (this.devicesLoaded) {
      await this.loadDevices();
    }
  }
}
