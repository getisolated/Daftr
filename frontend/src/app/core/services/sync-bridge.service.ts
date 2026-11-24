import { Injectable } from '@angular/core';
import type { CollectionDto } from '../bindings/CollectionDto';
import {
  CollectionConflicts, CollectionOfferMode, CollectionOffers, CollectionProgress, CollectionShare, CollectionStateEvent, CollectionSyncState,
  ConflictKeep, FolderSummary,
} from '../models/collection-sync.model';
import {
  DeviceRemoved, InitialSyncDone, LinkedDevice, NotesSynced, PairingState, PairingTicket, ProfileReceived, ProfileShares, SyncOverview, SyncStatus,
  TrustedDevice,
} from '../models/sync.model';
import { call, inDesktop, subscribe } from './desktop-api';

const NOT_IN_DESKTOP = 'Not in the desktop app';

/**
 * Commands and events of the LAN sync (Rust module `desktop::commands::sync`). A failure rejects with an `Error` whose
 * message is a stable key (`wrong_code`, `unreachable`…), translated by `syncErrorMessage`. Outside the desktop app the
 * reads are empty and the actions are refused.
 */
@Injectable({ providedIn: 'root' })
export class SyncBridgeService {
  private readonly available = inDesktop();

  get inDesktop(): boolean {
    return this.available;
  }

  async overview(): Promise<SyncOverview | null> {
    return this.available ? call<SyncOverview>('sync_state') : null;
  }

  async devices(): Promise<TrustedDevice[]> {
    return this.available ? call<TrustedDevice[]>('sync_devices') : [];
  }

  /** Every profile against every trusted device: what the matrix of the sync screen shows. */
  async profileShares(): Promise<ProfileShares | null> {
    return this.available ? call<ProfileShares>('sync_profile_shares') : null;
  }

  /** Shares a profile with a device (or stops): the device gets a clone of it, with no confirmation. */
  setProfileShared(profileId: string, deviceKey: string, shared: boolean): Promise<ProfileShares> {
    return this.required(() => call('sync_set_profile_shared', { profileId, deviceKey, shared }));
  }

  /** Opens the pairing mode: it only links two devices, whatever the profiles. */
  startPairing(): Promise<PairingTicket> {
    return this.required(() => call('sync_start_pairing'));
  }

  stopPairing(): Promise<void> {
    return this.required(() => call('sync_stop_pairing'));
  }

  /** Links to the device showing `code`; `address` (`IP:port`) when it is not found on the network. */
  pair(code: string, address: string | null): Promise<LinkedDevice> {
    return this.required(() => call('sync_pair', { code, address }));
  }

  /** Removes a device for good: only a new pairing lets it back. */
  removeDevice(deviceKey: string): Promise<void> {
    return this.required(() => call('sync_remove_device', { deviceKey }));
  }

  syncNow(): Promise<void> {
    return this.required(() => call('sync_now'));
  }

  /** Empty name: the host name. */
  setDeviceName(name: string): Promise<SyncOverview> {
    return this.required(() => call('sync_set_device_name', { name }));
  }

  // ── Shared collections (Rust module `desktop::commands::collection_sync`) ──

  /** Without a profile: the collections and sharings of every profile; the proposals are always global. */
  async collectionState(profileId: string | null = null): Promise<CollectionSyncState | null> {
    return this.available ? call<CollectionSyncState>('collection_sync_state', { profileId }) : null;
  }

  /** What a collection would share. */
  collectionSummary(root: string, profileId: string | null): Promise<FolderSummary> {
    return this.required(() => call('collection_sync_summary', { root, profileId }));
  }

  /** What a folder holds, to warn about a destination that already exists. */
  inspectFolder(path: string): Promise<FolderSummary> {
    return this.required(() => call('collection_sync_inspect_folder', { path }));
  }

  /** Shares or stops sharing a collection with one device; `null` once no device is left. */
  setCollectionDevice(root: string, deviceKey: string, shared: boolean, profileId: string): Promise<CollectionShare | null> {
    return this.required(() => call('collection_sync_set_device', { root, deviceKey, shared, profileId }));
  }

  stopCollection(profileId: string, syncId: string): Promise<void> {
    return this.required(() => call('collection_sync_stop', { profileId, syncId }));
  }

  /** Activates an entry "to configure" into a profile (active or not) and a folder. */
  activateCollection(deviceKey: string, syncId: string, profileId: string, destination: string, mode: CollectionOfferMode): Promise<void> {
    return this.required(() => call('collection_sync_activate', { deviceKey, syncId, profileId, destination, mode }));
  }

  ignoreCollection(deviceKey: string, syncId: string): Promise<void> {
    return this.required(() => call('collection_sync_ignore', { deviceKey, syncId }));
  }

  /** Without `folder`, a native folder dialog opens; `null` when none is chosen. */
  relinkCollection(root: string, folder: string | null): Promise<CollectionDto | null> {
    return this.required(() => call('collection_sync_relink', { root, folder }));
  }

  /** Native folder dialog; `null` when cancelled. */
  pickFolder(): Promise<string | null> {
    return this.required(() => call('collection_sync_pick_folder'));
  }

  resolveConflict(root: string, path: string, keep: ConflictKeep): Promise<void> {
    return this.required(() => call('collection_conflict_resolve', { root, path, keep }));
  }

  onCollectionOffer(callback: (offers: CollectionOffers) => void): () => void {
    return subscribe('sync:collection-offer', callback);
  }

  onCollectionProgress(callback: (progress: CollectionProgress) => void): () => void {
    return subscribe('sync:collection-progress', callback);
  }

  onCollectionState(callback: (event: CollectionStateEvent) => void): () => void {
    return subscribe('sync:collection-state', callback);
  }

  onCollectionConflicts(callback: (event: CollectionConflicts) => void): () => void {
    return subscribe('sync:collection-conflicts', callback);
  }

  onStatus(callback: (status: SyncStatus) => void): () => void {
    return subscribe('sync:status', callback);
  }

  onNotesSynced(callback: (event: NotesSynced) => void): () => void {
    return subscribe('sync:notes-synced', callback);
  }

  onInitialDone(callback: (event: InitialSyncDone) => void): () => void {
    return subscribe('sync:initial-done', callback);
  }

  onDeviceLinked(callback: (device: LinkedDevice) => void): () => void {
    return subscribe('sync:device-linked', callback);
  }

  onDeviceRemoved(callback: (event: DeviceRemoved) => void): () => void {
    return subscribe('sync:device-removed', callback);
  }

  onProfileReceived(callback: (event: ProfileReceived) => void): () => void {
    return subscribe('sync:profile-received', callback);
  }

  onPairing(callback: (state: PairingState) => void): () => void {
    return subscribe('sync:pairing', callback);
  }

  private required<T>(command: () => Promise<T>): Promise<T> {
    return this.available ? command() : Promise.reject(new Error(NOT_IN_DESKTOP));
  }
}
