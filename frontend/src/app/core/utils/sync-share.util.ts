import { CollectionOffer, CollectionShare, ProfileCollection } from '../models/collection-sync.model';
import { ProfileShares, TrustedDevice } from '../models/sync.model';

/** A column of the sharing matrix: a trusted device. */
export interface MatrixDevice {
  key: string;
  name: string;
}

/** A box of the matrix: whether the row is shared with the device, and whether the device has not taken it yet. */
export interface MatrixCell {
  deviceKey: string;
  checked: boolean;
  /** Shared, but nothing exchanged yet (the device is absent, or has not answered). */
  pending: boolean;
}

/** A row of the matrix: the profile itself, or one of its collections. */
export interface MatrixRow {
  kind: 'profile' | 'collection';
  /** Stable identity of the row. */
  id: string;
  label: string;
  /** Path of a collection on this device; `null` for the profile. */
  root: string | null;
  cells: MatrixCell[];
}

/** The two lists of "to configure" entries of a screen: under the selected profile, and without a profile. */
export interface OfferGroups {
  forProfile: CollectionOffer[];
  withoutProfile: CollectionOffer[];
}

/** Largest number shown in a badge. */
const BADGE_MAX = 9;

/** The columns: the trusted devices not removed, by name. */
export function matrixDevices(devices: readonly TrustedDevice[]): MatrixDevice[] {
  return devices
    .filter(device => device.revokedAt === null)
    .map(device => ({ key: device.key, name: device.name }))
    .sort((a, b) => a.name.localeCompare(b.name) || a.key.localeCompare(b.key));
}

/** The row of a profile: one box per device, unchecked for a device the profile knows nothing about. */
export function profileRow(profileId: string, label: string, shares: ProfileShares | null, devices: readonly MatrixDevice[]): MatrixRow {
  const known = shares?.profiles.find(share => share.profileId === profileId)?.devices ?? [];

  return {
    kind: 'profile',
    id: `profile:${profileId}`,
    label,
    root: null,
    cells: devices.map(device => {
      const cell = known.find(candidate => candidate.deviceKey === device.key);
      const checked = cell?.shared ?? false;

      return { deviceKey: device.key, checked, pending: checked && (cell?.pending ?? false) };
    }),
  };
}

/** The rows of the folder collections of a profile, by name; a device of the sharing checks its box. */
export function collectionRows(
  profileId: string, collections: readonly ProfileCollection[], shares: readonly CollectionShare[], devices: readonly MatrixDevice[],
): MatrixRow[] {
  return collections
    .filter(collection => collection.profileId === profileId)
    .sort((a, b) => a.name.localeCompare(b.name) || a.root.localeCompare(b.root))
    .map(collection => {
      const share = shares.find(candidate => candidate.profileId === profileId && candidate.root === collection.root);

      return {
        kind: 'collection',
        id: `collection:${profileId}:${collection.root}`,
        label: collection.name,
        root: collection.root,
        cells: devices.map(device => {
          const peer = share?.devices.find(candidate => candidate.key === device.key);

          return { deviceKey: device.key, checked: peer !== undefined, pending: peer?.state === 'offered' };
        }),
      };
    });
}

/**
 * Splits the "to configure" entries: the ones whose source profile is synchronized here with the selected profile, and the
 * ones with no known local profile (shown below the list whichever profile is selected). An entry for another profile is
 * left to that profile.
 */
export function groupOffers(
  offers: readonly CollectionOffer[], selectedProfileId: string, knownProfileIds: readonly string[],
): OfferGroups {
  const known = new Set(knownProfileIds);
  const forProfile: CollectionOffer[] = [];
  const withoutProfile: CollectionOffer[] = [];

  for (const offer of offers) {
    const local = offer.sourceProfileSyncedHere;

    if (local === null || !known.has(local)) {
      withoutProfile.push(offer);
    } else if (local === selectedProfileId) {
      forProfile.push(offer);
    }
  }

  return { forProfile, withoutProfile };
}

/** The local profile an entry starts on: the one synchronized with its source, otherwise the fallback (active) profile. */
export function defaultOfferProfile(offer: CollectionOffer, knownProfileIds: readonly string[], fallback: string): string {
  const local = offer.sourceProfileSyncedHere;

  return local !== null && knownProfileIds.includes(local) ? local : fallback;
}

/** Text of the badge of the sidebar row: nothing without entry, "9+" past nine. */
export function offerBadge(count: number): string | null {
  if (count <= 0) {
    return null;
  }

  return count > BADGE_MAX ? `${BADGE_MAX}+` : String(count);
}
