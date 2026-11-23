import { Language } from '../i18n/language.model';
import { translate, translatePlural } from '../i18n/translate';
import { TranslationKey } from '../i18n/translations';
import { Profile } from '../models/profile.model';
import {
  DeviceIcon, DeviceKind, OfferDestination, OfferDestinationOption, PairingTicket, ProfileOffer, ProfileSync, SpaceDevice, SyncIndicator,
  SyncProgress, SyncStatus, TrustedDevice,
} from '../models/sync.model';

/** Number of digits of a pairing code. */
export const PAIRING_CODE_LENGTH = 6;
/** Hexadecimal digits of the fingerprint shown under a device name. */
const SHORT_DEVICE_ID_LENGTH = 8;

/** Accessible names of the device icons. */
const DEVICE_ICON_LABELS: Readonly<Record<DeviceIcon, TranslationKey>> = {
  laptop: 'sync.device.laptop',
  desktop: 'sync.device.desktop',
  device: 'sync.device.unknown',
};

/**
 * Messages of the failed sync commands: the stable keys of the engine (`IpcError` of code `sync`, see the Rust module
 * `desktop::sync::errors`), then the refusals of the input checks (`presentation::sync`).
 */
const ERROR_KEYS: Readonly<Record<string, TranslationKey>> = {
  wrong_code: 'sync.error.wrong_code',
  not_pairing: 'sync.error.not_pairing',
  no_pairing_device: 'sync.error.no_pairing_device',
  unreachable: 'sync.error.unreachable',
  timeout: 'sync.error.timeout',
  not_local: 'sync.error.not_local',
  listen_failed: 'sync.error.listen_failed',
  discovery_failed: 'sync.error.discovery_failed',
  untrusted: 'sync.error.untrusted',
  unknown_offer: 'sync.error.unknown_offer',
  paused: 'sync.error.paused',
  fingerprint_mismatch: 'sync.error.fingerprint_mismatch',
  invalid_qr: 'sync.error.invalid_qr',
  protocol_version: 'sync.error.protocol_version',
  protocol_failed: 'sync.error.protocol_failed',
  stopped: 'sync.error.stopped',
  storage_failed: 'sync.error.storage_failed',
  'profile already synchronized': 'sync.error.alreadySynced',
  'invalid pairing code': 'sync.error.invalidCode',
  'invalid address': 'sync.error.invalidAddress',
  'invalid device name': 'sync.error.invalidDeviceName',
  'invalid port': 'sync.error.invalidPort',
};

/** Causes of the error state of the indicator (`SyncStatusDto.reason`). */
const REASON_KEYS: Readonly<Record<string, TranslationKey>> = {
  listen_failed: 'sync.reason.listen_failed',
  discovery_failed: 'sync.reason.discovery_failed',
};

/** Message of a failed sync command, translated; an unknown cause keeps its text after a generic sentence. */
export function syncErrorMessage(error: unknown, language: Language): string {
  const message = error instanceof Error ? error.message : String(error);
  const key = ERROR_KEYS[message.trim()];

  return key ? translate(language, key) : translate(language, 'sync.error.generic', { message });
}

/** Translated cause of the error state, for the tooltip of the indicator. */
export function syncReason(reason: string | null, language: Language): string {
  const key = reason ? REASON_KEYS[reason] : undefined;

  return key ? translate(language, key) : translate(language, 'sync.reason.unknown', { reason: reason ?? '?' });
}

/** How the indicator shows a state; `null` while it is unknown (outside the desktop app, before the first answer). */
export function syncIndicator(status: SyncStatus | null, language: Language): SyncIndicator | null {
  if (!status) {
    return null;
  }

  switch (status.state) {
    case 'disabled':
      return { state: status.state, tone: 'off', label: translate(language, 'sync.status.disabled'), reason: null };
    case 'paused':
      return { state: status.state, tone: 'off', label: translate(language, 'sync.status.paused'), reason: null };
    case 'upToDate':
      return { state: status.state, tone: 'ok', label: translatePlural(language, 'sync.status.upToDate', status.devices ?? 0), reason: null };
    case 'syncing':
      return { state: status.state, tone: 'busy', label: translate(language, 'sync.status.syncing'), reason: null };
    case 'noDevice':
      return { state: status.state, tone: 'warning', label: translate(language, 'sync.status.noDevice'), reason: null };
    case 'error':
      return { state: status.state, tone: 'error', label: translate(language, 'sync.status.error'), reason: syncReason(status.reason, language) };
  }
}

/** Time left before `expiresAt` (ms since the Unix epoch), never negative. */
export function remainingMs(expiresAt: number, now: number): number {
  return Math.max(0, expiresAt - now);
}

/** "4:59": minutes and seconds, the seconds rounded up so that "0:00" means expired. */
export function formatCountdown(remaining: number): string {
  const seconds = Math.ceil(Math.max(0, remaining) / 1000);
  const minutes = Math.floor(seconds / 60);

  return `${minutes}:${String(seconds % 60).padStart(2, '0')}`;
}

/** The digits typed for a pairing code (spaces and dashes copied along are dropped), at most six. */
export function normalizePairingCode(input: string): string {
  return input.replace(/\D/g, '').slice(0, PAIRING_CODE_LENGTH);
}

export function isPairingCode(code: string): boolean {
  return code.length === PAIRING_CODE_LENGTH && /^\d+$/.test(code);
}

/** "123 456": easier to read aloud and to copy by eye. */
export function formatPairingCode(code: string): string {
  return code.length === PAIRING_CODE_LENGTH ? `${code.slice(0, 3)} ${code.slice(3)}` : code;
}

/**
 * Addresses of a pairing QR code (`daftr-pair:1:<addresses>:<fingerprint>:<code>`, IPv6 between brackets), shown under
 * the code for a manual entry; empty for another content.
 */
export function pairingAddresses(uri: string): string {
  const parts = uri.split(':');

  return parts[0] === 'daftr-pair' && parts.length >= 5 ? parts.slice(2, -2).join(':').split(',').join(', ') : '';
}

/**
 * An SVG as an image URL: shown by an `<img>`, the markup is never inserted in the page, so no sanitizer bypass is
 * needed and nothing in it can run.
 */
export function svgDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/**
 * A new pairing code is needed to keep one on screen: the mode closed (linking, expiry) or its countdown ran out, and no
 * new code is on its way (`waiting`: starting, failed, or the user is typing the code of another device).
 */
export function needsNewPairingCode(ticket: PairingTicket | null, remaining: number, waiting: boolean): boolean {
  return !waiting && (ticket === null || remaining <= 0);
}

/**
 * "4F2A-9C1B": the start of the fingerprint of a device key (the one of the mDNS announcement and the QR code), enough
 * to tell apart two devices with the same name. The MAC address is not used: the protocol does not know it, and it
 * changes per Wi-Fi network and per network card.
 */
export function shortDeviceId(fingerprint: string): string {
  const start = fingerprint.slice(0, SHORT_DEVICE_ID_LENGTH).toUpperCase();
  const half = SHORT_DEVICE_ID_LENGTH / 2;

  return start.length === SHORT_DEVICE_ID_LENGTH ? `${start.slice(0, half)}-${start.slice(half)}` : start;
}

/** Icon of a device: the generic one when its kind is unknown (detection failed, device of an older version). */
export function deviceIcon(kind: DeviceKind): DeviceIcon {
  switch (kind) {
    case 'laptop':
      return 'laptop';
    case 'desktop':
      return 'desktop';
    case 'unknown':
      return 'device';
  }
}

/** Accessible name of a device icon ("Laptop"). */
export function deviceIconLabel(icon: DeviceIcon, language: Language): string {
  return translate(language, DEVICE_ICON_LABELS[icon]);
}

/** Profiles checked when "Link a device" opens: the active one. */
export function initialPairingProfiles(profiles: Profile[], activeId: string): string[] {
  return profiles.some(profile => profile.id === activeId) ? [activeId] : [];
}

/** Profiles a proposed profile can be merged into: the ones not synchronized yet (a profile belongs to one space). */
export function mergeCandidates(profiles: Profile[], syncedProfileIds: readonly string[]): Profile[] {
  const synced = new Set(syncedProfileIds);

  return profiles.filter(profile => !synced.has(profile.id));
}

/** Choices for a proposed profile: a new profile, a merge into each profile not synchronized yet, or nothing. */
export function offerDestinationOptions(
  offer: ProfileOffer,
  profiles: Profile[],
  syncedProfileIds: readonly string[],
  profileName: (profile: Profile) => string,
  language: Language,
): OfferDestinationOption[] {
  const merges = mergeCandidates(profiles, syncedProfileIds).map(profile => ({
    value: `merge:${profile.id}`,
    label: translate(language, 'sync.offer.merge', { name: profileName(profile) }),
  }));

  return [
    { value: 'new', label: translate(language, 'sync.offer.new', { name: offer.name }) },
    ...merges,
    { value: 'ignore', label: translate(language, 'sync.offer.ignore') },
  ];
}

/** The choice behind a value of `offerDestinationOptions`. */
export function parseOfferDestination(value: string): OfferDestination {
  if (value.startsWith('merge:')) {
    return { choice: 'merge', profileId: value.slice('merge:'.length) };
  }

  return { choice: value === 'ignore' ? 'ignore' : 'new', profileId: null };
}

/** The other devices a profile is synchronized with ("Synchronized with: …"). */
export function sharedDevices(sync: ProfileSync | null): SpaceDevice[] {
  return (sync?.devices ?? []).filter(device => !device.isSelf && !device.revoked);
}

/** Trusted devices a profile can still be proposed to: not removed, and not already sharing it. */
export function offerTargets(devices: TrustedDevice[], sync: ProfileSync | null): TrustedDevice[] {
  const sharing = new Set(sharedDevices(sync).map(device => device.key));

  return devices.filter(device => device.revokedAt === null && !sharing.has(device.key));
}

/** Most recent exchange with a device, over the profiles it shares (SQLite UTC dates sort as text). */
export function lastSyncAt(device: TrustedDevice): string | null {
  return device.profiles.reduce<string | null>((latest, profile) => {
    const at = profile.lastSyncAt;

    return at !== null && (latest === null || at > latest) ? at : latest;
  }, null);
}

/** Ids of the profiles a device still shares (not stopped with "Stop with…"). */
export function sharedProfileIds(device: TrustedDevice): string[] {
  return device.profiles.filter(profile => !profile.revoked).map(profile => profile.profileId);
}

/** "Initial sync: 120 / 450 notes", or a waiting message before the first count. */
export function initialSyncLabel(progress: SyncProgress | null, language: Language): string {
  if (!progress || progress.total === 0) {
    return translate(language, 'sync.offer.waiting');
  }

  return translate(language, 'sync.offer.progress', { done: Math.min(progress.done, progress.total), total: progress.total });
}

/** "Changed on Laptop", or on another device when the event does not name it. */
export function remoteChangeLabel(deviceName: string | null, language: Language): string {
  return deviceName ? translate(language, 'sync.editor.changedOn', { device: deviceName }) : translate(language, 'sync.editor.changedElsewhere');
}

/** Banner of a note deleted on another device. */
export function remoteDeletionLabel(deviceName: string | null, language: Language): string {
  return deviceName ? translate(language, 'sync.editor.deletedOn', { device: deviceName }) : translate(language, 'sync.editor.deletedElsewhere');
}
