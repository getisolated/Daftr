import { profileStorageKey } from '../utils/profile.util';

/** Profile whose data the window shows; set by `main.ts` before Angular starts, so every service reads the right keys. */
let sessionProfileId: string | null = null;

export function setSessionProfile(profileId: string | null): void {
  sessionProfileId = profileId;
}

/** localStorage key of a value tied to the data of the active profile (open tabs, sidebar, expanded collections). */
export function sessionStorageKey(base: string): string {
  return profileStorageKey(base, sessionProfileId);
}
