import { Injectable } from '@angular/core';
import { Profile, ProfilesState, TransferItems, TransferResult } from '../models/profile.model';
import { call, callWithBytes, inDesktop, subscribe } from './desktop-api';

const NOT_IN_DESKTOP = 'Not in the desktop app';

/** The profiles and the active one, read before Angular starts (`main.ts`); `null` outside the desktop app. */
export async function fetchProfilesState(): Promise<ProfilesState | null> {
  return inDesktop() ? call<ProfilesState>('profiles_state') : null;
}

/** Access to the profiles of the main process. Outside the desktop app there is only the default profile, unchanged. */
@Injectable({ providedIn: 'root' })
export class ProfilesBridgeService {
  private readonly available = inDesktop();

  state(): Promise<ProfilesState | null> {
    return fetchProfilesState();
  }

  unlock(password: string): Promise<void> {
    return this.required(() => call('profiles_unlock', { password }));
  }

  switchTo(id: string, password: string | null): Promise<void> {
    return this.required(() => call('profiles_switch', { id, password }));
  }

  create(name: string | null): Promise<Profile> {
    return this.required(() => call('profiles_create', { name }));
  }

  rename(id: string, name: string | null): Promise<Profile> {
    return this.required(() => call('profiles_rename', { id, name }));
  }

  /** The icon travels as raw bytes, its profile in a header. */
  setIcon(id: string, bytes: Uint8Array): Promise<Profile> {
    return this.required(() => callWithBytes('profiles_set_icon', bytes, { 'profile-id': id }));
  }

  removeIcon(id: string): Promise<Profile> {
    return this.required(() => call('profiles_remove_icon', { id }));
  }

  /** Bytes of the icon, `null` without one. */
  async icon(id: string): Promise<Uint8Array<ArrayBuffer> | null> {
    if (!this.available) {
      return null;
    }

    const buffer = await call<ArrayBuffer>('profiles_icon', { id });

    return buffer.byteLength > 0 ? new Uint8Array(buffer) : null;
  }

  setPassword(id: string, current: string | null, next: string | null): Promise<Profile> {
    return this.required(() => call('profiles_set_password', { id, current, next }));
  }

  delete(id: string, password: string | null): Promise<void> {
    return this.required(() => call('profiles_delete', { id, password }));
  }

  transfer(target: string, items: TransferItems): Promise<TransferResult> {
    return this.required(() => call('profiles_transfer', { target, ...items }));
  }

  /** The active profile changed from another window (the quick-note overlay reloads). */
  onProfileChanged(callback: () => void): () => void {
    return subscribe('profiles:changed', callback);
  }

  private required<T>(command: () => Promise<T>): Promise<T> {
    return this.available ? command() : Promise.reject(new Error(NOT_IN_DESKTOP));
  }
}
