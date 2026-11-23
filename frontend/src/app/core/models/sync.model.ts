import type { InitialSyncDoneDto } from '../bindings/InitialSyncDoneDto';
import type { LinkedDeviceDto } from '../bindings/LinkedDeviceDto';
import type { NotesSyncedDto } from '../bindings/NotesSyncedDto';
import type { OfferReceivedDto } from '../bindings/OfferReceivedDto';
import type { PairingStateDto } from '../bindings/PairingStateDto';
import type { PairingTicketDto } from '../bindings/PairingTicketDto';
import type { ProfileOfferDto } from '../bindings/ProfileOfferDto';
import type { ProfileSyncDto } from '../bindings/ProfileSyncDto';
import type { SpaceDeviceDto } from '../bindings/SpaceDeviceDto';
import type { SyncProgressDto } from '../bindings/SyncProgressDto';
import type { SyncStateDto } from '../bindings/SyncStateDto';
import type { SyncStatusDto } from '../bindings/SyncStatusDto';
import type { SyncStatusKind } from '../bindings/SyncStatusKind';
import type { TrustedDeviceDto } from '../bindings/TrustedDeviceDto';

/** The DTOs of the sync (crate `presentation`, `dto/sync.rs`), under the names the renderer uses. */
export type SyncOverview = SyncStateDto;
export type SyncStatus = SyncStatusDto;
export type SyncStatusState = SyncStatusKind;
export type TrustedDevice = TrustedDeviceDto;
export type LinkedDevice = LinkedDeviceDto;
export type PairingTicket = PairingTicketDto;
export type PairingState = PairingStateDto;
export type ProfileSync = ProfileSyncDto;
export type SpaceDevice = SpaceDeviceDto;
export type ProfileOffer = ProfileOfferDto;
export type OfferReceived = OfferReceivedDto;
export type NotesSynced = NotesSyncedDto;
export type SyncProgress = SyncProgressDto;
export type InitialSyncDone = InitialSyncDoneDto;

/** Answer to a proposed profile (`sync_answer_offer`). */
export type OfferChoice = 'new' | 'merge' | 'ignore';

/** Where a proposed profile goes on this device; `profileId` with `merge` only. */
export interface OfferDestination {
  choice: OfferChoice;
  profileId: string | null;
}

/** An entry of the destination list of a proposed profile. */
export interface OfferDestinationOption {
  /** `new`, `ignore` or `merge:<profile id>`: the value of the drop-down. */
  value: string;
  label: string;
}

/** How the indicator of the sidebar draws a state. */
export type SyncTone = 'off' | 'ok' | 'busy' | 'warning' | 'error';

/** The sync state as shown by the indicator: its tone, its text, and for an error the cause in a tooltip. */
export interface SyncIndicator {
  state: SyncStatusState;
  tone: SyncTone;
  label: string;
  /** Translated cause of an error, `null` otherwise. */
  reason: string | null;
}

/** Step of "Link a device" / "Link to a device" in the sync settings. */
export type PairingStep = 'idle' | 'chooseProfiles' | 'showCode' | 'enterCode';
