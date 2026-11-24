import type { CollectionConflictDto } from '../bindings/CollectionConflictDto';
import type { CollectionConflictsDto } from '../bindings/CollectionConflictsDto';
import type { CollectionOfferDto } from '../bindings/CollectionOfferDto';
import type { CollectionOffersDto } from '../bindings/CollectionOffersDto';
import type { CollectionProgressDto } from '../bindings/CollectionProgressDto';
import type { CollectionShareDeviceDto } from '../bindings/CollectionShareDeviceDto';
import type { CollectionShareDto } from '../bindings/CollectionShareDto';
import type { CollectionShareStatusKind } from '../bindings/CollectionShareStatusKind';
import type { CollectionStateEventDto } from '../bindings/CollectionStateEventDto';
import type { CollectionSyncStateDto } from '../bindings/CollectionSyncStateDto';
import type { FolderSummaryDto } from '../bindings/FolderSummaryDto';

/** The DTOs of the collection sync (crate `presentation`, `dto/collection_sync.rs`), under the names the renderer uses. */
export type CollectionShare = CollectionShareDto;
export type CollectionShareDevice = CollectionShareDeviceDto;
export type CollectionShareStatus = CollectionShareStatusKind;
export type CollectionConflict = CollectionConflictDto;
export type CollectionOffer = CollectionOfferDto;
export type CollectionOffers = CollectionOffersDto;
export type CollectionProgress = CollectionProgressDto;
export type CollectionStateEvent = CollectionStateEventDto;
export type CollectionConflicts = CollectionConflictsDto;
export type CollectionSyncState = CollectionSyncStateDto;
export type FolderSummary = FolderSummaryDto;

/** Answer to a proposed collection (`collection_sync_answer_offer`). */
export type CollectionOfferMode = 'receive' | 'merge' | 'decline';

/** Side kept when a conflict is resolved. */
export type ConflictKeep = 'mine' | 'other';

/** Tone of the badge of a shared collection in the sidebar. */
export type ShareBadgeTone = 'ok' | 'busy' | 'warning' | 'paused' | 'error' | 'pending';

/** Glyph of the badge: ⟳ (up to date, syncing), ⚠, ⏸, ✕ and a clock for a proposal not answered yet. */
export type ShareBadgeGlyph = 'sync' | 'warning' | 'pause' | 'cross' | 'clock';

/** A shared collection as the sidebar shows it. */
export interface CollectionShareBadge {
  status: CollectionShareStatus;
  tone: ShareBadgeTone;
  glyph: ShareBadgeGlyph;
  /** "37 / 142" while syncing, `null` otherwise. */
  progress: string | null;
  tooltip: string;
}

/** The sharing assistant: asked for a collection of the sidebar, or from the profile view where the user picks one. */
export interface ShareAssistantRequest {
  /** Collection to share; `null`: the assistant asks which one. */
  root: string | null;
  /** Where it was opened from: the "link a device" hint then leads to the devices view, or stays in it. */
  from: 'sidebar' | 'devices';
}
