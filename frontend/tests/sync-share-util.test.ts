import { describe, it, expect } from 'vitest';
import type { CollectionOffer, CollectionShare, ProfileCollection } from '../src/app/core/models/collection-sync.model';
import type { ProfileShares, TrustedDevice } from '../src/app/core/models/sync.model';
import {
  collectionRows, defaultOfferProfile, groupOffers, matrixDevices, offerBadge, profileRow,
} from '../src/app/core/utils/sync-share.util';

function trusted(key: string, name: string, overrides: Partial<TrustedDevice> = {}): TrustedDevice {
  return {
    key, fingerprint: 'ab'.repeat(16), name, kind: 'laptop', origin: 'paired', addedAt: '2026-11-20 10:00:00', revokedAt: null, present: true, connected: true,
    profiles: [],
    ...overrides,
  };
}

function offer(name: string, syncedHere: string | null, key = 'laptop'): CollectionOffer {
  return {
    device: { key, name: 'Laptop' }, syncId: `sync-${name}`, name, files: 3, bytes: 1024, sourceProfileId: 'src', sourceProfileName: 'Perso',
    sourceProfileSyncedHere: syncedHere, suggestedDestination: `D:\\${name}`,
  };
}

function collection(profileId: string, name: string, syncId: string | null = null): ProfileCollection {
  return { profileId, root: `C:\\${profileId}\\${name}`, name, syncId };
}

function share(profileId: string, name: string, devices: { key: string; state?: 'offered' | 'active' }[]): CollectionShare {
  return {
    profileId, syncId: `s-${name}`, root: `C:\\${profileId}\\${name}`, name, status: 'upToDate', files: 1, done: 0, total: 0, conflicts: [], ignored: [],
    truncated: false,
    devices: devices.map(device => ({ key: device.key, name: device.key, state: device.state ?? 'active', connected: true, syncing: false, lastSyncAt: null })),
  };
}

const DEVICES = matrixDevices([trusted('b', 'Bureau'), trusted('p', 'Portable')]);

describe('matrixDevices', () => {
  it('lists the trusted devices not removed, by name', () => {
    const devices = [trusted('z', 'Zeta'), trusted('gone', 'Old', { revokedAt: '2026-11-21 08:00:00' }), trusted('a', 'Alpha')];

    expect(matrixDevices(devices)).toEqual([{ key: 'a', name: 'Alpha' }, { key: 'z', name: 'Zeta' }]);
  });
});

describe('profileRow', () => {
  const shares: ProfileShares = {
    profiles: [
      { profileId: 'perso', spaceId: 'sp', devices: [{ deviceKey: 'b', shared: true, pending: false }, { deviceKey: 'p', shared: true, pending: true }] },
      { profileId: 'work', spaceId: null, devices: [{ deviceKey: 'b', shared: false, pending: false }] },
    ],
  };

  it('checks the devices the profile is shared with, and marks the ones that received nothing yet', () => {
    const row = profileRow('perso', 'Perso', shares, DEVICES);

    expect(row.kind).toBe('profile');
    expect(row.cells).toEqual([
      { deviceKey: 'b', checked: true, pending: false },
      { deviceKey: 'p', checked: true, pending: true },
    ]);
  });

  it('leaves a device the profile knows nothing about unchecked', () => {
    expect(profileRow('work', 'Work', shares, DEVICES).cells.map(cell => cell.checked)).toEqual([false, false]);
    expect(profileRow('gone', 'Gone', shares, DEVICES).cells.every(cell => !cell.checked)).toBe(true);
    expect(profileRow('perso', 'Perso', null, DEVICES).cells.every(cell => !cell.checked)).toBe(true);
  });

  it('does not call pending a box that is not checked', () => {
    const odd: ProfileShares = { profiles: [{ profileId: 'x', spaceId: null, devices: [{ deviceKey: 'b', shared: false, pending: true }] }] };

    expect(profileRow('x', 'X', odd, DEVICES).cells[0]?.pending).toBe(false);
  });
});

describe('collectionRows', () => {
  const collections = [collection('perso', 'WMS'), collection('perso', 'LAURE'), collection('work', 'Docs')];

  it('keeps the collections of the profile, by name', () => {
    expect(collectionRows('perso', collections, [], DEVICES).map(row => row.label)).toEqual(['LAURE', 'WMS']);
  });

  it('checks the devices of the sharing of each collection', () => {
    const rows = collectionRows('perso', collections, [share('perso', 'LAURE', [{ key: 'p' }, { key: 'b', state: 'offered' }])], DEVICES);

    expect(rows[0]?.cells).toEqual([
      { deviceKey: 'b', checked: true, pending: true },
      { deviceKey: 'p', checked: true, pending: false },
    ]);
    expect(rows[1]?.cells.every(cell => !cell.checked)).toBe(true);
  });

  it('does not take the sharing of another profile for the same folder', () => {
    const other = share('work', 'LAURE', [{ key: 'b' }]);

    expect(collectionRows('perso', collections, [{ ...other, root: collections[1]?.root ?? '' }], DEVICES)[0]?.cells.every(cell => !cell.checked)).toBe(true);
  });

  it('gives a stable identity and the path of each row', () => {
    const [row] = collectionRows('work', collections, [], DEVICES);

    expect(row?.id).toBe('collection:work:C:\\work\\Docs');
    expect(row?.root).toBe('C:\\work\\Docs');
  });
});

describe('groupOffers', () => {
  const known = ['perso', 'work'];

  it('puts an entry under the profile synchronized with its source', () => {
    const groups = groupOffers([offer('a', 'perso'), offer('b', 'work')], 'perso', known);

    expect(groups.forProfile.map(item => item.name)).toEqual(['a']);
    expect(groups.withoutProfile).toEqual([]);
  });

  it('puts an entry with no local profile in the group without profile, whichever profile is selected', () => {
    const entries = [offer('a', null), offer('b', 'gone')];

    expect(groupOffers(entries, 'perso', known).withoutProfile.map(item => item.name)).toEqual(['a', 'b']);
    expect(groupOffers(entries, 'work', known).withoutProfile.map(item => item.name)).toEqual(['a', 'b']);
    expect(groupOffers(entries, 'work', known).forProfile).toEqual([]);
  });

  it('leaves an entry of another profile to that profile', () => {
    const groups = groupOffers([offer('a', 'work')], 'perso', known);

    expect(groups).toEqual({ forProfile: [], withoutProfile: [] });
  });
});

describe('defaultOfferProfile', () => {
  it('starts on the synchronized profile, otherwise on the fallback', () => {
    expect(defaultOfferProfile(offer('a', 'work'), ['perso', 'work'], 'perso')).toBe('work');
    expect(defaultOfferProfile(offer('a', null), ['perso', 'work'], 'perso')).toBe('perso');
    expect(defaultOfferProfile(offer('a', 'gone'), ['perso', 'work'], 'perso')).toBe('perso');
  });
});

describe('offerBadge', () => {
  it('shows the number of entries, nothing without one, and caps at nine', () => {
    expect(offerBadge(0)).toBeNull();
    expect(offerBadge(-1)).toBeNull();
    expect(offerBadge(1)).toBe('1');
    expect(offerBadge(9)).toBe('9');
    expect(offerBadge(10)).toBe('9+');
  });
});
