import type { ProfileDto } from '../bindings/ProfileDto';
import type { ProfilesStateDto } from '../bindings/ProfilesStateDto';
import type { TransferResultDto } from '../bindings/TransferResultDto';

/** A profile: its own notes, folders and collections; the settings are shared by all profiles. */
export type Profile = ProfileDto;

/** The profiles and the active one (`locked`: it waits for its master password). */
export type ProfilesState = ProfilesStateDto;

/** What left the active profile during a transfer. */
export type TransferResult = TransferResultDto;

/** Identifier of the profile holding the data from before the profiles. */
export const DEFAULT_PROFILE_ID = 'default';

/** Largest side of a profile icon, in pixels. */
export const PROFILE_ICON_SIZE = 256;

/** What the user transfers to another profile. */
export interface TransferItems {
  noteIds: number[];
  folderIds: number[];
  collections: string[];
}

/** Change of the icon asked in the settings dialog. */
export type ProfileIconChange = { kind: 'keep' } | { kind: 'remove' } | { kind: 'set'; image: Blob };

/** Change of the master password asked in the settings dialog. */
export type ProfilePasswordChange =
  | { kind: 'keep' }
  | { kind: 'set'; current: string | null; next: string }
  | { kind: 'remove'; current: string };

/** What the settings dialog of a profile edits. */
export interface ProfileDraft {
  name: string;
  icon: ProfileIconChange;
  password: ProfilePasswordChange;
}

/** Source rectangle (square, centred) of an image and the side of the resized icon. */
export interface SquareCrop {
  sx: number;
  sy: number;
  size: number;
  target: number;
}

/** Dialog opened from the profile menu of the sidebar. */
export type ProfileSwitcherDialog =
  | { kind: 'settings'; profile: Profile | null; devices?: boolean }
  | { kind: 'password'; purpose: 'switch' | 'delete'; profile: Profile }
  | { kind: 'confirmDelete'; profile: Profile };
