import { Language } from '../i18n/language.model';
import { translate, translatePlural } from '../i18n/translate';
import { Collection } from '../models/collection.model';
import {
  CollectionConflict, CollectionOffer, CollectionOfferMode, CollectionShare, CollectionShareBadge, FolderSummary,
} from '../models/collection-sync.model';

/** Bytes per unit of the sizes shown ("3,2 Mo"). */
const KIBI = 1024;
const SECOND_MS = 1000;
const SIZE_UNITS: Readonly<Record<Language, readonly string[]>> = {
  en: ['B', 'KB', 'MB', 'GB'],
  fr: ['o', 'Ko', 'Mo', 'Go'],
};

/** "3,2 Mo" / "3.2 MB": one decimal from the megabytes up, none below. */
export function formatBytes(bytes: number, language: Language): string {
  const units = SIZE_UNITS[language];
  let value = Math.max(0, bytes);
  let unit = 0;

  while (value >= KIBI && unit < units.length - 1) {
    value /= KIBI;
    unit++;
  }

  const digits = unit >= 2 ? 1 : 0;
  const text = new Intl.NumberFormat(language, { maximumFractionDigits: digits }).format(value);

  return `${text} ${units[unit]}`;
}

/** "il y a 2 min": a SQLite UTC date (`YYYY-MM-DD HH:MM:SS`) against `now` (ms since the epoch); `null` when unreadable. */
export function relativeAgo(at: string | null, now: number, language: Language): string | null {
  if (at === null) {
    return null;
  }

  const time = Date.parse(`${at.replace(' ', 'T')}Z`);

  if (Number.isNaN(time)) {
    return null;
  }

  const seconds = Math.max(0, Math.round((now - time) / SECOND_MS));
  const formatter = new Intl.RelativeTimeFormat(language, { numeric: 'auto' });

  if (seconds < 60) {
    return formatter.format(0, 'second');
  }

  const minutes = Math.round(seconds / 60);

  if (minutes < 60) {
    return formatter.format(-minutes, 'minute');
  }

  const hours = Math.round(minutes / 60);

  if (hours < 24) {
    return formatter.format(-hours, 'hour');
  }

  return formatter.format(-Math.round(hours / 24), 'day');
}

/** The sharing of a collection, by its path on this device. */
export function shareOfRoot(shares: readonly CollectionShare[], root: string): CollectionShare | null {
  return shares.find(share => share.root === root) ?? null;
}

/** Names of the devices of a sharing in a state (`offered`: not answered yet, `active`: sharing), joined for a sentence. */
function deviceNames(share: CollectionShare, state: 'offered' | 'active', onlyAbsent: boolean): string {
  return share.devices
    .filter(device => device.state === state && (!onlyAbsent || !device.connected))
    .map(device => device.name)
    .join(', ');
}

/** The most recent complete exchange of a sharing (SQLite UTC dates sort as text). */
export function lastSyncOfShare(share: CollectionShare): string | null {
  return share.devices.reduce<string | null>((latest, device) => {
    const at = device.lastSyncAt;

    return at !== null && (latest === null || at > latest) ? at : latest;
  }, null);
}

/** "37 / 142": the progress of an exchange, never above its total. */
export function progressLabel(done: number, total: number): string {
  return `${Math.min(Math.max(0, done), total)} / ${total}`;
}

/** How the sidebar draws a shared collection: the glyph and colour of its badge, and what the tooltip says. */
export function shareBadge(share: CollectionShare, now: number, language: Language): CollectionShareBadge {
  switch (share.status) {
    case 'missingFolder':
      return { status: share.status, tone: 'error', glyph: 'cross', progress: null, tooltip: translate(language, 'csync.status.missingFolder') };
    case 'syncing': {
      const progress = share.total > 0 ? progressLabel(share.done, share.total) : null;
      const tooltip = progress === null
        ? translate(language, 'csync.status.syncing')
        : translate(language, 'csync.status.syncingProgress', { progress });

      return { status: share.status, tone: 'busy', glyph: 'sync', progress, tooltip };
    }
    case 'conflicts':
      return {
        status: share.status,
        tone: 'warning',
        glyph: 'warning',
        progress: null,
        tooltip: translatePlural(language, 'csync.status.conflicts', share.conflicts.length),
      };
    case 'waiting': {
      const names = deviceNames(share, 'active', true) || deviceNames(share, 'active', false);

      return { status: share.status, tone: 'paused', glyph: 'pause', progress: null, tooltip: translate(language, 'csync.status.waiting', { devices: names }) };
    }
    case 'offered':
      return {
        status: share.status,
        tone: 'pending',
        glyph: 'clock',
        progress: null,
        tooltip: translate(language, 'csync.status.offered', { devices: deviceNames(share, 'offered', false) }),
      };
    case 'upToDate': {
      const names = deviceNames(share, 'active', false);
      const ago = relativeAgo(lastSyncOfShare(share), now, language);
      const tooltip = ago === null
        ? translate(language, 'csync.status.upToDate', { devices: names })
        : translate(language, 'csync.status.upToDateAgo', { devices: names, ago });

      return { status: share.status, tone: 'ok', glyph: 'sync', progress: null, tooltip };
    }
  }
}

/** The name of the device a state "waits for" (shown next to "En attente de …"). */
export function waitingDevices(share: CollectionShare): string {
  return deviceNames(share, 'active', true) || deviceNames(share, 'active', false);
}

/** The conflict whose copy is `path` of the collection `root`, among all the sharings; `null` for a normal file. */
export function conflictOfFile(shares: readonly CollectionShare[], root: string, path: string): CollectionConflict | null {
  return shareOfRoot(shares, root)?.conflicts.find(conflict => conflict.path === path) ?? null;
}

/** Paths of the conflict copies of a collection, to mark them in the tree. */
export function conflictPaths(shares: readonly CollectionShare[], root: string): ReadonlySet<string> {
  return new Set((shareOfRoot(shares, root)?.conflicts ?? []).map(conflict => conflict.path));
}

/** A share replaced by its new state, added when new, removed when `share` is `null` (the sharing no longer exists). */
export function applyShareState(
  shares: readonly CollectionShare[], profileId: string, syncId: string, share: CollectionShare | null,
): CollectionShare[] {
  const others = shares.filter(candidate => !(candidate.profileId === profileId && candidate.syncId === syncId));

  if (share === null) {
    return others;
  }

  const position = shares.findIndex(candidate => candidate.profileId === profileId && candidate.syncId === syncId);

  // Keeps the place of a known sharing: the list does not reorder at each event
  return position < 0 ? [...others, share] : [...others.slice(0, position), share, ...others.slice(position)];
}

/** The progress of an exchange applied to its sharing; unchanged for an unknown one or a finished exchange. */
export function applyProgress(
  shares: readonly CollectionShare[], profileId: string, syncId: string, done: number, total: number,
): CollectionShare[] {
  return shares.map(share => {
    if (share.profileId !== profileId || share.syncId !== syncId) {
      return share;
    }

    // A missing folder stays so: no exchange can run on it
    const status = share.status === 'missingFolder' || done >= total ? share.status : 'syncing';

    return { ...share, done, total, status };
  });
}

/**
 * The conflict copies of a sharing replaced. New conflicts show at once (they come before the waiting and up-to-date
 * states); an empty list needs the backend to say what the state becomes, hence the `true` returned for a reload.
 */
export function applyConflicts(
  shares: readonly CollectionShare[], profileId: string, syncId: string, conflicts: readonly CollectionConflict[],
): { shares: CollectionShare[]; reload: boolean } {
  let reload = false;
  const next = shares.map(share => {
    if (share.profileId !== profileId || share.syncId !== syncId) {
      return share;
    }

    const quiet = share.status === 'upToDate' || share.status === 'waiting' || share.status === 'offered';
    const status = conflicts.length > 0 && quiet ? 'conflicts' : share.status;
    reload = conflicts.length === 0 && share.status === 'conflicts';

    return { ...share, conflicts: [...conflicts], status };
  });

  return { shares: next, reload };
}

/** Stable identity of a proposal. */
export function offerKey(offer: Pick<CollectionOffer, 'device' | 'syncId'>): string {
  return `${offer.device.key}:${offer.syncId}`;
}

/** Proposals by device name, then collection name: the order of the list stays the same between two events. */
export function sortOffers(offers: readonly CollectionOffer[]): CollectionOffer[] {
  return [...offers].sort((a, b) => a.device.name.localeCompare(b.device.name) || a.name.localeCompare(b.name) || a.syncId.localeCompare(b.syncId));
}

/** The pending proposals once a device proposed (again): its previous ones are replaced by the new list. */
export function mergeDeviceOffers(
  pending: readonly CollectionOffer[], deviceKey: string, proposed: readonly CollectionOffer[],
): CollectionOffer[] {
  return sortOffers([...pending.filter(offer => offer.device.key !== deviceKey), ...proposed]);
}

/** The proposals not seen yet, to announce them once (a device proposes again at each connection). */
export function newOffers(offers: readonly CollectionOffer[], known: ReadonlySet<string>): CollectionOffer[] {
  return offers.filter(offer => !known.has(offerKey(offer)));
}

/**
 * How a proposal is answered for a destination: an existing folder with `.md` files is merged, anything else is received
 * as it is (the backend refuses a non-empty folder without a merge).
 */
export function acceptMode(folder: FolderSummary | null): CollectionOfferMode {
  return folder !== null && folder.exists && !folder.empty ? 'merge' : 'receive';
}

/** The destination holds files already: the warning "this folder already exists" shows. */
export function needsMergeWarning(folder: FolderSummary | null): boolean {
  return folder !== null && folder.exists && !folder.empty;
}

/** Folder collections that can still be shared (a file collection cannot, a shared one is already). */
export function shareableCollections(collections: readonly Collection[], shares: readonly CollectionShare[]): Collection[] {
  return collections.filter(collection => collection.kind === 'folder' && shareOfRoot(shares, collection.path) === null);
}

/** Files the summary leaves out because of their size, the others being unreadable or colliding by case. */
export function tooLargeCount(folder: FolderSummary): number {
  return folder.ignored.filter(file => file.reason === 'tooLarge').length;
}

/** The sharings of a profile, by name. */
export function sharesOfProfile(shares: readonly CollectionShare[], profileId: string): CollectionShare[] {
  return shares.filter(share => share.profileId === profileId).sort((a, b) => a.name.localeCompare(b.name));
}

/** The pending proposals of a profile. */
export function offersOfProfile(offers: readonly CollectionOffer[], profileId: string): CollectionOffer[] {
  return offers.filter(offer => offer.profileId === profileId);
}
