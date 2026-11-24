import { Injectable, computed, inject, signal } from '@angular/core';
import { DEFAULT_PROFILE_ID, Profile, ProfileDraft } from '../models/profile.model';
import { iconMimeType } from '../utils/profile.util';
import { PendingSavesService } from './pending-saves.service';
import { ProfilesBridgeService } from './profiles-bridge.service';

/**
 * The profiles: list, icons, switching, settings. Switching reloads the window: every service then starts again on the
 * data of the new profile (its session keys included, see sessionStorageKey).
 */
@Injectable({ providedIn: 'root' })
export class ProfilesService {
  private readonly bridge = inject(ProfilesBridgeService);
  private readonly pendingSaves = inject(PendingSavesService);

  private readonly _profiles = signal<Profile[]>([]);
  private readonly _activeId = signal<string>(DEFAULT_PROFILE_ID);
  /** Object URLs of the icons, by profile id. */
  private readonly _icons = signal<ReadonlyMap<string, string>>(new Map());

  readonly profiles = this._profiles.asReadonly();
  readonly activeId = this._activeId.asReadonly();
  readonly active = computed<Profile | null>(() => this._profiles().find(profile => profile.id === this._activeId()) ?? null);
  /** Profiles the active one can switch or transfer to. */
  readonly others = computed<Profile[]>(() => this._profiles().filter(profile => profile.id !== this._activeId()));

  /** Reads the profiles and their icons. */
  async load(): Promise<void> {
    const state = await this.bridge.state();

    if (!state) {
      return;
    }

    this._profiles.set(state.profiles);
    this._activeId.set(state.activeId);
    await this.loadIcons(state.profiles);
  }

  /** URL of the icon of a profile, `null` without one (the avatar shows its initial). */
  iconUrl(id: string): string | null {
    return this._icons().get(id) ?? null;
  }

  /** Opens a profile (its password asked first when it has one), then reloads the window on its data. */
  async switchTo(id: string, password: string | null): Promise<void> {
    await this.pendingSaves.flushAll();
    await this.bridge.switchTo(id, password);
    location.reload();
  }

  /** Opens the active profile locked at launch, then reloads the window on its data. */
  async unlock(password: string): Promise<void> {
    await this.bridge.unlock(password);
    location.reload();
  }

  /** Creates a profile as set in the dialog and switches to it. */
  async create(draft: ProfileDraft): Promise<void> {
    const created = await this.bridge.create(draft.name.trim() || null);
    await this.applyIcon(created.id, draft);
    await this.applyPassword(created.id, draft);

    const password = draft.password.kind === 'set' ? draft.password.next : null;
    await this.switchTo(created.id, password);
  }

  /** Saves the settings of a profile. */
  async update(id: string, draft: ProfileDraft): Promise<void> {
    try {
      await this.bridge.rename(id, draft.name.trim() || null);
      await this.applyIcon(id, draft);
      await this.applyPassword(id, draft);
    } finally {
      await this.load();
    }
  }

  /**
   * Deletes a profile: its data goes to the trash. Deleting the active one opens another profile in its place (the
   * default profile starts again empty when it was the only one), so the window reloads.
   */
  async delete(id: string, password: string | null): Promise<void> {
    const wasActive = id === this.activeId();

    if (wasActive) {
      await this.pendingSaves.flushAll();
    }

    await this.bridge.delete(id, password);

    if (wasActive) {
      location.reload();
    } else {
      await this.load();
    }
  }

  private async applyIcon(id: string, draft: ProfileDraft): Promise<void> {
    if (draft.icon.kind === 'set') {
      const bytes = new Uint8Array(await draft.icon.image.arrayBuffer());
      await this.bridge.setIcon(id, bytes);
    } else if (draft.icon.kind === 'remove') {
      await this.bridge.removeIcon(id);
    }
  }

  private async applyPassword(id: string, draft: ProfileDraft): Promise<void> {
    const change = draft.password;

    if (change.kind === 'set') {
      await this.bridge.setPassword(id, change.current, change.next);
    } else if (change.kind === 'remove') {
      await this.bridge.setPassword(id, change.current, null);
    }
  }

  private async loadIcons(profiles: Profile[]): Promise<void> {
    const icons = new Map<string, string>();

    for (const profile of profiles.filter(candidate => candidate.hasIcon)) {
      const url = await this.iconObjectUrl(profile.id);

      if (url) {
        icons.set(profile.id, url);
      }
    }

    this._icons().forEach(url => URL.revokeObjectURL(url));
    this._icons.set(icons);
  }

  private async iconObjectUrl(id: string): Promise<string | null> {
    try {
      const bytes = await this.bridge.icon(id);
      const type = bytes ? iconMimeType(bytes) : null;

      return bytes && type ? URL.createObjectURL(new Blob([bytes], { type })) : null;
    } catch (error) {
      console.error('[Profiles] Icon not loaded:', error);
      return null;
    }
  }
}
