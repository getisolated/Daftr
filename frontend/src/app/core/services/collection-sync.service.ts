import { Injectable, OnDestroy, computed, effect, inject, signal, untracked } from '@angular/core';
import { I18nService } from '../i18n/i18n.service';
import {
  CollectionConflicts, CollectionOffer, CollectionOfferMode, CollectionOffers, CollectionProgress, CollectionShare, CollectionStateEvent,
  ConflictKeep, FolderSummary, ShareAssistantRequest,
} from '../models/collection-sync.model';
import {
  applyConflicts, applyProgress, applyShareState, conflictPaths, mergeDeviceOffers, newOffers, offerKey, shareOfRoot, sortOffers,
} from '../utils/collection-sync.util';
import { CollectionsService } from './collections.service';
import { SyncBridgeService } from './sync-bridge.service';
import { SyncService } from './sync.service';
import { ToastService } from './toast.service';

/**
 * State of the shared collections for the interface: the sharings of every synchronized profile (status, devices,
 * conflict copies), the proposals waiting for an answer, and the two dialogs (sharing assistant, proposal). The engine
 * runs in the main process; this service follows its events and calls its commands.
 */
@Injectable({ providedIn: 'root' })
export class CollectionSyncService implements OnDestroy {
  private readonly bridge = inject(SyncBridgeService);
  private readonly sync = inject(SyncService);
  private readonly collections = inject(CollectionsService);
  private readonly toasts = inject(ToastService);
  private readonly i18n = inject(I18nService);

  private readonly _shares = signal<CollectionShare[]>([]);
  private readonly _offers = signal<CollectionOffer[]>([]);
  private readonly _receivedFolder = signal('');
  private readonly _assistant = signal<ShareAssistantRequest | null>(null);
  /** Key of the proposal shown by the dialog; "Later" closes it and the proposal stays in `offers`. */
  private readonly _offerInDialog = signal<string | null>(null);
  /** Proposals already announced by a notification: a device proposes again at each connection. */
  private readonly announced = new Set<string>();
  private readonly unsubscribers: (() => void)[];

  readonly shares = this._shares.asReadonly();
  /** Every proposal waiting for an answer, including the ones put off with "Later". */
  readonly offers = this._offers.asReadonly();
  /** Where the received collections arrive by default (the setting, or `Documents/Daftr`). */
  readonly receivedFolder = this._receivedFolder.asReadonly();
  readonly assistant = this._assistant.asReadonly();
  readonly offerInDialog = computed(() => this._offers().find(offer => offerKey(offer) === this._offerInDialog()) ?? null);

  constructor() {
    this.unsubscribers = [
      this.bridge.onCollectionOffer(event => this.onOffer(event)),
      this.bridge.onCollectionProgress(progress => this.onProgress(progress)),
      this.bridge.onCollectionState(event => this.onState(event)),
      this.bridge.onCollectionConflicts(event => void this.onConflicts(event)),
    ];
    void this.load();

    // Removing a device or stopping a profile share takes it out of its collections, without an event of their own
    effect(() => {
      if (this.sync.membershipChanges() > 0) {
        untracked(() => { void this.load(); });
      }
    });
  }

  ngOnDestroy(): void {
    this.unsubscribers.forEach(unsubscribe => unsubscribe());
  }

  /** The sharing of a collection of this device, `null` when it is not shared. */
  shareOf(root: string): CollectionShare | null {
    return shareOfRoot(this._shares(), root);
  }

  /** Conflict copies of a collection, by path, to mark them in its tree. */
  conflictPathsOf(root: string): ReadonlySet<string> {
    return conflictPaths(this._shares(), root);
  }

  errorMessage(error: unknown): string {
    return this.sync.errorMessage(error);
  }

  /** Reads the sharings, the proposals and the folder of the received collections. */
  async load(): Promise<void> {
    try {
      const state = await this.bridge.collectionState();

      if (state) {
        this._shares.set(state.shares);
        this._offers.set(sortOffers(state.offers));
        this._receivedFolder.set(state.receivedFolder);
        // Already there at start: no notification for what the user could have seen before
        state.offers.forEach(offer => this.announced.add(offerKey(offer)));
      }
    } catch (error) {
      console.error('[CollectionSync] State not read:', error);
    }
  }

  summary(root: string): Promise<FolderSummary> {
    return this.bridge.collectionSummary(root);
  }

  inspectFolder(path: string): Promise<FolderSummary> {
    return this.bridge.inspectFolder(path);
  }

  /** Native folder dialog; `null` when cancelled. */
  pickFolder(): Promise<string | null> {
    return this.bridge.pickFolder();
  }

  openAssistant(root: string | null, from: ShareAssistantRequest['from']): void {
    this._assistant.set({ root, from });
  }

  closeAssistant(): void {
    this._assistant.set(null);
  }

  openOffer(offer: CollectionOffer): void {
    this._offerInDialog.set(offerKey(offer));
  }

  /** "Later": the dialog closes, the proposal stays in the list of the pending ones. */
  closeOffer(): void {
    this._offerInDialog.set(null);
  }

  /** Proposes the collection to the devices (they must already share the active profile). */
  async share(root: string, deviceKeys: string[]): Promise<CollectionShare> {
    const share = await this.bridge.shareCollection(root, deviceKeys);
    this._shares.update(shares => applyShareState(shares, share.profileId, share.syncId, share));

    return share;
  }

  /** Cuts the link: the files stay on every device. */
  async stop(share: CollectionShare): Promise<void> {
    await this.bridge.stopCollection(share.profileId, share.syncId);
    this._shares.update(shares => applyShareState(shares, share.profileId, share.syncId, null));
    await this.load();
  }

  /** Answers a proposal; an accepted one makes its collection appear in the sidebar, which syncs as it fills. */
  async answerOffer(offer: CollectionOffer, destination: string | null, mode: CollectionOfferMode): Promise<void> {
    await this.bridge.answerCollectionOffer(offer.device.key, offer.syncId, destination, mode);
    this._offers.update(offers => offers.filter(candidate => offerKey(candidate) !== offerKey(offer)));

    if (offerKey(offer) === this._offerInDialog()) {
      this._offerInDialog.set(null);
    }

    await this.collections.reload();
    await this.load();
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
    const fresh = newOffers(event.offers, this.announced);
    event.offers.forEach(offer => this.announced.add(offerKey(offer)));
    this._offers.update(pending => mergeDeviceOffers(pending, event.device.key, event.offers));

    const first = fresh[0];

    if (!first) {
      return;
    }

    const message = fresh.length === 1
      ? this.i18n.t('csync.toast.offer', {
        device: event.device.name,
        name: first.name,
        files: this.i18n.plural('csync.files', first.files),
      })
      : this.i18n.plural('csync.toast.offerMany', fresh.length, { device: event.device.name });

    this.toasts.show(message, { label: this.i18n.t('csync.toast.view'), run: () => this.openOffer(first) });
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
