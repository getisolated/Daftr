import { Injectable, OnDestroy, computed, effect, inject, signal, untracked } from '@angular/core';
import { I18nService } from '../i18n/i18n.service';
import {
  InitialSyncDone, LinkedDevice, NotesSynced, PairingTicket, ProfileReceived, ProfileShares, SyncOverview, SyncStatus, TrustedDevice,
} from '../models/sync.model';
import { profileDisplayName } from '../utils/profile.util';
import { syncErrorMessage, syncIndicator } from '../utils/sync.util';
import { ProfilesService } from './profiles.service';
import { SettingsService } from './settings.service';
import { SyncBridgeService } from './sync-bridge.service';
import { ToastService } from './toast.service';

/** Category of the sync in the settings editor (see SETTING_DEFINITIONS). */
export const SYNC_SETTINGS_CATEGORY = 'sync.settings.category';

/** `sync_pair` answers and `sync:device-linked` arrives for the same linking: a single notification. */
const LINK_NOTICE_MS = 5000;

/**
 * State of the LAN sync for the interface: status of the indicator, overview and trusted devices, pairing mode, the
 * sharing of each profile, the "Synchronization" screen, in-app notifications. The engine runs in the main process;
 * this service only follows its events and calls its commands.
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
  /** The pairing mode is wanted (the screen is open): a code that arrives after it closed is not left open. */
  private pairingWanted = false;
  /** The pairing code was renewed after too many wrong attempts. */
  private readonly _pairingRenewed = signal(false);
  private readonly _profileShares = signal<ProfileShares | null>(null);
  private readonly _screenOpen = signal(false);
  private readonly notesSyncedListeners = new Set<(event: NotesSynced) => void>();
  private readonly unsubscribers: (() => void)[];
  /** A pushed status is newer than the one of a pending `sync_state`. */
  private statusPushed = false;
  private devicesLoaded = false;
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
  readonly pairingRenewed = this._pairingRenewed.asReadonly();
  /** Every profile against every trusted device; `null` until read. */
  readonly profileShares = this._profileShares.asReadonly();
  /** The "Synchronization" screen is open. */
  readonly screenOpen = this._screenOpen.asReadonly();

  constructor() {
    this.unsubscribers = [
      this.bridge.onStatus(status => this.onStatus(status)),
      this.bridge.onNotesSynced(event => this.notesSyncedListeners.forEach(listener => listener(event))),
      this.bridge.onInitialDone(event => void this.onInitialDone(event)),
      this.bridge.onDeviceLinked(device => void this.onDeviceLinked(device)),
      this.bridge.onProfileReceived(event => void this.onProfileReceived(event)),
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

  async syncNow(): Promise<void> {
    await this.bridge.syncNow();
  }

  async setDeviceName(name: string): Promise<void> {
    this._overview.set(await this.bridge.setDeviceName(name.trim()));
    await this.settings.load();
  }

  openScreen(): void {
    this._screenOpen.set(true);
  }

  closeScreen(): void {
    this._screenOpen.set(false);
  }

  /** Reads the sharing of each profile (the matrix of the screen). */
  async loadProfileShares(): Promise<void> {
    try {
      this._profileShares.set(await this.bridge.profileShares());
    } catch (error) {
      console.error('[Sync] Profile shares not read:', error);
    }
  }

  /** Ticks or unticks a profile for a device: the device gets a clone of it, or stops receiving it. */
  async setProfileShared(profileId: string, deviceKey: string, shared: boolean): Promise<void> {
    this._profileShares.set(await this.bridge.setProfileShared(profileId, deviceKey, shared));
    await this.reloadDevices();
  }

  /** "Link a device": opens the pairing mode, which only links the two devices. */
  async startPairing(): Promise<void> {
    this.pairingWanted = true;
    const ticket = await this.bridge.startPairing();

    // Stopped while opening (the screen closed): the mode must not stay open with its code shown nowhere
    if (!this.pairingWanted) {
      await this.bridge.stopPairing();
      return;
    }

    this._pairingRenewed.set(false);
    this._pairing.set(ticket);
  }

  async stopPairing(): Promise<void> {
    this.pairingWanted = false;
    this._pairing.set(null);
    await this.bridge.stopPairing();
  }

  /** "Link to a device": the code shown by the other device, and its address when it is not found on the network. */
  async pair(code: string, address: string | null): Promise<void> {
    const device = await this.bridge.pair(code, address);
    this.noticeLinked(device);
    await Promise.all([this.refresh(), this.reloadDevices()]);
  }

  /** Removes a device for good; its shares end with it (the engine says so by events). */
  async removeDevice(deviceKey: string): Promise<void> {
    await this.bridge.removeDevice(deviceKey);
    await Promise.all([this.reloadDevices(), this.loadProfileShares()]);
  }

  private onStatus(status: SyncStatus): void {
    this.statusPushed = true;
    this._status.set(status);
    // Presence and connections of the devices change with the status (no event of their own), and so do the shares not
    // taken yet by an absent device
    void this.reloadDevices();

    if (this._screenOpen()) {
      void this.loadProfileShares();
    }
  }

  private async onInitialDone(event: InitialSyncDone): Promise<void> {
    // A profile received from a device is already announced by its own event: this one tells its first exchange is over
    await this.profiles.load();
    const profile = this.profiles.profiles().find(candidate => candidate.id === event.profileId);
    const name = profile ? profileDisplayName(profile, this.i18n.t) : event.profileId;
    this.toasts.show(this.i18n.t('sync.toast.initialDone', { profile: name }));
    await Promise.all([this.refresh(), this.reloadDevices(), this.loadProfileShares()]);
  }

  private async onDeviceLinked(device: LinkedDevice): Promise<void> {
    this.noticeLinked(device);
    await Promise.all([this.refresh(), this.reloadDevices()]);
  }

  /** A profile shared by another device arrived as a clone: the toast offers to switch to it. */
  private async onProfileReceived(event: ProfileReceived): Promise<void> {
    await Promise.all([this.profiles.load(), this.loadProfileShares()]);

    this.toasts.show(this.i18n.t('sync.toast.profileReceived', { name: event.name, device: event.device.name }), {
      label: this.i18n.t('sync.toast.switch'),
      run: () => void this.switchToReceived(event.profileId),
    });
  }

  private async switchToReceived(profileId: string): Promise<void> {
    try {
      // The clone arrives without a master password
      await this.profiles.switchTo(profileId, null);
    } catch (error) {
      console.error('[Sync] Switch to the received profile failed:', error);
      this.toasts.show(this.i18n.t('sync.toast.switchFailed'));
    }
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
