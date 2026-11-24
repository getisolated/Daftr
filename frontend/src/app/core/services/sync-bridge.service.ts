import { Injectable } from '@angular/core';
import type { CollectionDto } from '../bindings/CollectionDto';
import {
  CollectionConflicts, CollectionOfferMode, CollectionOffers, CollectionProgress, CollectionShare, CollectionStateEvent, CollectionSyncState,
  ConflictKeep, FolderSummary,
} from '../models/collection-sync.model';
import {
  InitialSyncDone, LinkedDevice, NotesSynced, OfferChoice, OfferReceived, PairingState, PairingTicket, ProfileSync, SyncOverview,
  SyncProgress, SyncStatus, TrustedDevice,
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

  async profile(profileId: string): Promise<ProfileSync | null> {
    return this.available ? call<ProfileSync>('sync_profile', { profileId }) : null;
  }

  /** Opens the pairing mode for these profiles (their spaces are created if needed). */
  startPairing(profileIds: string[]): Promise<PairingTicket> {
    return this.required(() => call('sync_start_pairing', { profileIds }));
  }

  stopPairing(): Promise<void> {
    return this.required(() => call('sync_stop_pairing'));
  }

  /** Links to the device showing `code`; `address` (`IP:port`) when it is not found on the network. */
  pair(code: string, address: string | null): Promise<LinkedDevice> {
    return this.required(() => call('sync_pair', { code, address }));
  }

  answerOffer(deviceKey: string, spaceId: string, choice: OfferChoice, profileId: string | null): Promise<void> {
    return this.required(() => call('sync_answer_offer', { deviceKey, spaceId, choice, profileId }));
  }

  /** "Sync with…": proposes a profile to a trusted device (sent as soon as it shows up on the network). */
  offerProfile(profileId: string, deviceKey: string): Promise<void> {
    return this.required(() => call('sync_offer_profile', { profileId, deviceKey }));
  }

  stopSharing(profileId: string, deviceKey: string): Promise<void> {
    return this.required(() => call('sync_stop_sharing', { profileId, deviceKey }));
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

  async collectionState(): Promise<CollectionSyncState | null> {
    return this.available ? call<CollectionSyncState>('collection_sync_state') : null;
  }

  /** What a collection of the active profile would share. */
  collectionSummary(root: string): Promise<FolderSummary> {
    return this.required(() => call('collection_sync_summary', { root }));
  }

  /** What a folder holds, to warn about a destination that already exists. */
  inspectFolder(path: string): Promise<FolderSummary> {
    return this.required(() => call('collection_sync_inspect_folder', { path }));
  }

  /** The devices must already share the active profile. */
  shareCollection(root: string, deviceKeys: string[]): Promise<CollectionShare> {
    return this.required(() => call('collection_sync_share', { root, deviceKeys }));
  }

  stopCollection(profileId: string, syncId: string): Promise<void> {
    return this.required(() => call('collection_sync_stop', { profileId, syncId }));
  }

  /** `destination` is required except for `decline`. */
  answerCollectionOffer(deviceKey: string, syncId: string, destination: string | null, mode: CollectionOfferMode): Promise<void> {
    return this.required(() => call('collection_sync_answer_offer', { deviceKey, syncId, destination, mode }));
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

  onProgress(callback: (progress: SyncProgress) => void): () => void {
    return subscribe('sync:progress', callback);
  }

  onInitialDone(callback: (event: InitialSyncDone) => void): () => void {
    return subscribe('sync:initial-done', callback);
  }

  onDeviceLinked(callback: (device: LinkedDevice) => void): () => void {
    return subscribe('sync:device-linked', callback);
  }

  onOfferReceived(callback: (offer: OfferReceived) => void): () => void {
    return subscribe('sync:offer-received', callback);
  }

  onPairing(callback: (state: PairingState) => void): () => void {
    return subscribe('sync:pairing', callback);
  }

  private required<T>(command: () => Promise<T>): Promise<T> {
    return this.available ? command() : Promise.reject(new Error(NOT_IN_DESKTOP));
  }
}
