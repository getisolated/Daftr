import { Injectable, OnDestroy, computed, inject, signal } from '@angular/core';
import { I18nService } from '../i18n/i18n.service';
import {
  CollectionConflicts, CollectionOffer, CollectionOfferMode, CollectionOffers, CollectionProgress, CollectionShare, CollectionStateEvent,
  ConflictKeep, FolderSummary, ProfileCollection,
} from '../models/collection-sync.model';
import {
  applyConflicts, applyProgress, applyShareState, conflictPaths, newOffers, offerKey, shareOfRoot, sortOffers,
} from '../utils/collection-sync.util';
import { CollectionsService } from './collections.service';
import { ProfilesService } from './profiles.service';
import { SyncBridgeService } from './sync-bridge.service';
import { SyncService } from './sync.service';
import { ToastService } from './toast.service';

/**
 * State of the shared collections for the interface: the folder collections of every profile (for the sync screen), their
 * sharings (status, devices, conflict copies) and the entries "to configure" waiting for an answer. The engine runs in the
 * main process; this service follows its events and calls its commands.
 */
@Injectable({ providedIn: 'root' })
export class CollectionSyncService implements OnDestroy {
  private readonly bridge = inject(SyncBridgeService);
  private readonly sync = inject(SyncService);
  private readonly collections = inject(CollectionsService);
  private readonly profiles = inject(ProfilesService);
  private readonly toasts = inject(ToastService);
  private readonly i18n = inject(I18nService);

  private readonly _shares = signal<CollectionShare[]>([]);
  private readonly _collections = signal<ProfileCollection[]>([]);
  private readonly _offers = signal<CollectionOffer[]>([]);
  private readonly _receivedFolder = signal('');
  /** Entries already announced by a notification: a device proposes again at each connection. */
  private readonly announced = new Set<string>();
  private readonly unsubscribers: (() => void)[];

  readonly shares = this._shares.asReadonly();
  /** The folder collections of every profile, shared or not. */
  readonly profileCollections = this._collections.asReadonly();
  /** Every entry "to configure", whichever profile it may land in. */
  readonly offers = this._offers.asReadonly();
  /** Where the received collections arrive by default (the setting, or `Documents/Daftr`). */
  readonly receivedFolder = this._receivedFolder.asReadonly();
  readonly pendingCount = computed(() => this._offers().length);

  constructor() {
    this.unsubscribers = [
      this.bridge.onCollectionOffer(event => this.onOffer(event)),
      this.bridge.onCollectionProgress(progress => this.onProgress(progress)),
      this.bridge.onCollectionState(event => this.onState(event)),
      this.bridge.onCollectionConflicts(event => void this.onConflicts(event)),
    ];
    void this.load();
  }

  ngOnDestroy(): void {
    this.unsubscribers.forEach(unsubscribe => unsubscribe());
  }

  /** The sharing of a collection of the active profile, `null` when it is not shared. */
  shareOf(root: string): CollectionShare | null {
    return shareOfRoot(this._shares(), root, this.profiles.activeId());
  }

  /** Conflict copies of a collection, by path, to mark them in its tree. */
  conflictPathsOf(root: string): ReadonlySet<string> {
    return conflictPaths(this._shares(), root);
  }

  errorMessage(error: unknown): string {
    return this.sync.errorMessage(error);
  }

  /** Reads the collections, their sharings, the entries "to configure" and the folder of the received collections. */
  async load(): Promise<void> {
    try {
      const state = await this.bridge.collectionState();

      if (state) {
        this._shares.set(state.shares);
        this._collections.set(state.collections);
        this._offers.set(sortOffers(state.offers));
        this._receivedFolder.set(state.receivedFolder);
        // Already there at start: no notification for what the user could have seen before
        state.offers.forEach(offer => this.announced.add(offerKey(offer)));
      }
    } catch (error) {
      console.error('[CollectionSync] State not read:', error);
    }
  }

  summary(root: string, profileId: string): Promise<FolderSummary> {
    return this.bridge.collectionSummary(root, profileId);
  }

  inspectFolder(path: string): Promise<FolderSummary> {
    return this.bridge.inspectFolder(path);
  }

  /** Native folder dialog; `null` when cancelled. */
  pickFolder(): Promise<string | null> {
    return this.bridge.pickFolder();
  }

  /** Shares a collection with a device, or stops sharing it with that one; the new state comes back with the answer. */
  async setDevice(profileId: string, root: string, deviceKey: string, shared: boolean): Promise<void> {
    const share = await this.bridge.setCollectionDevice(root, deviceKey, shared, profileId);

    if (share) {
      this._shares.update(shares => applyShareState(shares, share.profileId, share.syncId, share));
    } else {
      this._shares.update(shares => shares.filter(candidate => !(candidate.profileId === profileId && candidate.root === root)));
    }
  }

  /** Cuts the link: the files stay on every device. */
  async stop(share: CollectionShare): Promise<void> {
    await this.bridge.stopCollection(share.profileId, share.syncId);
    this._shares.update(shares => applyShareState(shares, share.profileId, share.syncId, null));
    await this.load();
  }

  /**
   * Activates an entry "to configure": the collection is created in the profile (active or not) and starts exchanging. When
   * the profile is the active one, its collections are read again to show the new one in the sidebar.
   */
  async activate(offer: CollectionOffer, profileId: string, destination: string, mode: CollectionOfferMode): Promise<void> {
    await this.bridge.activateCollection(offer.device.key, offer.syncId, profileId, destination, mode);
    this._offers.update(offers => offers.filter(candidate => offerKey(candidate) !== offerKey(offer)));

    if (profileId === this.profiles.activeId()) {
      await this.collections.reload();
    }

    await this.load();
  }

  /** Refuses an entry: it leaves the list, and the other device is not asked anything. */
  async ignore(offer: CollectionOffer): Promise<void> {
    await this.bridge.ignoreCollection(offer.device.key, offer.syncId);
    this._offers.update(offers => offers.filter(candidate => offerKey(candidate) !== offerKey(offer)));
  }

  /** The folder of a shared collection moved: the user shows the new one (native dialog). */
  async relink(root: string): Promise<void> {
    const row = await this.bridge.relinkCollection(root, null);

    if (row) {
      await this.collections.reopen(root, row.path);
      await this.load();
    }
  }

  /** Keeps a side of a conflict: the copy leaves (recycle bin), its tab closes. */
  async resolveConflict(root: string, path: string, keep: ConflictKeep): Promise<void> {
    await this.bridge.resolveConflict(root, path, keep);
    this.collections.closeFileTabs({ root, path });
    await this.load();
  }

  private onOffer(event: CollectionOffers): void {
    const fresh = newOffers(event.added, this.announced);
    event.pending.forEach(offer => this.announced.add(offerKey(offer)));
    this._offers.set(sortOffers(event.pending));

    const first = fresh[0];

    if (!first) {
      return;
    }

    const message = fresh.length === 1
      ? this.i18n.t('csync.toast.offer', {
        device: first.device.name,
        name: first.name,
        files: this.i18n.plural('csync.files', first.files),
      })
      : this.i18n.plural('csync.toast.offerMany', fresh.length, { device: first.device.name });

    this.toasts.show(message, { label: this.i18n.t('csync.toast.view'), run: () => this.sync.openScreen() });
  }

  private onProgress(progress: CollectionProgress): void {
    this._shares.update(shares => applyProgress(shares, progress.profileId, progress.syncId, progress.done, progress.total));
  }

  private onState(event: CollectionStateEvent): void {
    this._shares.update(shares => applyShareState(shares, event.profileId, event.syncId, event.share));
  }

  private async onConflicts(event: CollectionConflicts): Promise<void> {
    const result = applyConflicts(this._shares(), event.profileId, event.syncId, event.conflicts);
    this._shares.set(result.shares);

    // All resolved: the state that follows (up to date, waiting…) is the backend's to say
    if (result.reload) {
      await this.load();
    }
  }
}
