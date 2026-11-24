import type { DeviceKindDto } from '../bindings/DeviceKindDto';
import type { DeviceRemovedDto } from '../bindings/DeviceRemovedDto';
import type { InitialSyncDoneDto } from '../bindings/InitialSyncDoneDto';
import type { LinkedDeviceDto } from '../bindings/LinkedDeviceDto';
import type { NotesSyncedDto } from '../bindings/NotesSyncedDto';
import type { PairingStateDto } from '../bindings/PairingStateDto';
import type { PairingTicketDto } from '../bindings/PairingTicketDto';
import type { ProfileReceivedDto } from '../bindings/ProfileReceivedDto';
import type { ProfileShareDeviceDto } from '../bindings/ProfileShareDeviceDto';
import type { ProfileShareDto } from '../bindings/ProfileShareDto';
import type { ProfileSharesDto } from '../bindings/ProfileSharesDto';
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
export type ProfileShares = ProfileSharesDto;
export type ProfileShare = ProfileShareDto;
export type ProfileShareDevice = ProfileShareDeviceDto;
export type ProfileReceived = ProfileReceivedDto;
export type DeviceRemoved = DeviceRemovedDto;
export type NotesSynced = NotesSyncedDto;
export type SyncProgress = SyncProgressDto;
export type InitialSyncDone = InitialSyncDoneDto;
export type DeviceKind = DeviceKindDto;

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

/** Icon drawn for a device. */
export type DeviceIcon = 'laptop' | 'desktop' | 'device';
