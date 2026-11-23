import type { Translator } from '../i18n/translate';
import type { TranslationKey } from '../i18n/translations';
import { DEFAULT_PROFILE_ID, Profile, ProfilePasswordChange, SquareCrop } from '../models/profile.model';

const PNG_SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/**
 * localStorage key of a session value (tabs, sidebar…) for a profile. The default profile keeps the key used before
 * the profiles existed, so its session survives the update.
 */
export function profileStorageKey(base: string, profileId: string | null): string {
  if (profileId === null || profileId === DEFAULT_PROFILE_ID) {
    return base;
  }

  return `${base}@${profileId}`;
}

/** Largest centred square of a `width` × `height` image, scaled down to `max` pixels at most (never up). */
export function squareCrop(width: number, height: number, max: number): SquareCrop {
  const size = Math.max(1, Math.floor(Math.min(width, height)));
  const sx = Math.floor((width - size) / 2);
  const sy = Math.floor((height - size) / 2);

  return { sx: Math.max(0, sx), sy: Math.max(0, sy), size, target: Math.min(size, max) };
}

/** Type of an icon sent back by the main process, told by its signature (PNG or WebP). */
export function iconMimeType(bytes: Uint8Array): 'image/png' | 'image/webp' | null {
  if (PNG_SIGNATURE.every((byte, index) => bytes[index] === byte)) {
    return 'image/png';
  }

  const riff = String.fromCharCode(...bytes.slice(0, 4));
  const webp = String.fromCharCode(...bytes.slice(8, 12));

  return riff === 'RIFF' && webp === 'WEBP' ? 'image/webp' : null;
}

/** First letter of the name, upper-cased, for the placeholder of the icon; `null` without a name. */
export function profileInitial(name: string | null): string | null {
  const first = Array.from(name?.trim() ?? '')[0];

  return first ? first.toLocaleUpperCase() : null;
}

/** Name shown for a profile: its own, or a translated placeholder when it has none. */
export function profileDisplayName(profile: Pick<Profile, 'name' | 'isDefault'>, t: Translator): string {
  if (profile.name) {
    return profile.name;
  }

  return profile.isDefault ? t('profiles.defaultName') : t('profiles.unnamed');
}

/** Password fields of the settings dialog. */
export interface PasswordFields {
  current: string;
  next: string;
  confirm: string;
  remove: boolean;
}

/**
 * Password change asked by the fields, or the reason it cannot be applied. An empty new password keeps the current
 * state; changing or removing an existing password needs the current one (checked by the main process).
 */
export function passwordChange(isProtected: boolean, fields: PasswordFields): ProfilePasswordChange | TranslationKey {
  if (isProtected && fields.remove) {
    return fields.current ? { kind: 'remove', current: fields.current } : 'profiles.settings.currentRequired';
  }

  if (!fields.next) {
    return { kind: 'keep' };
  }

  if (fields.next !== fields.confirm) {
    return 'profiles.settings.passwordMismatch';
  }

  if (isProtected && !fields.current) {
    return 'profiles.settings.currentRequired';
  }

  return { kind: 'set', current: isProtected ? fields.current : null, next: fields.next };
}

/** Message of a failed profile command, translated when the main process refused it for a known reason. */
export function profileErrorMessage(error: unknown, t: Translator): string {
  const message = error instanceof Error ? error.message : String(error);

  return message === 'Wrong password' ? t('profiles.error.wrongPassword') : t('profiles.error.failed', { message });
}
