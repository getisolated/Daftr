import { describe, it, expect } from 'vitest';
import type { Collection } from '../src/app/core/models/collection.model';
import type { CollectionOffer, CollectionShare, FolderSummary } from '../src/app/core/models/collection-sync.model';
import {
  acceptMode, applyConflicts, applyProgress, applyShareState, conflictOfFile, conflictPaths, formatBytes, lastSyncOfShare, mergeDeviceOffers,
  needsMergeWarning, newOffers, offerKey, offersOfProfile, progressLabel, relativeAgo, shareBadge, shareOfRoot, shareableCollections, sharesOfProfile,
  sortOffers, tooLargeCount, waitingDevices,
} from '../src/app/core/utils/collection-sync.util';
import { syncErrorMessage } from '../src/app/core/utils/sync.util';

const NOW = Date.parse('2026-11-24T12:00:00Z');

function share(overrides: Partial<CollectionShare> = {}): CollectionShare {
  return {
    profileId: 'default', syncId: 's1', root: 'C:\\notes\\wms', name: 'WMS', status: 'upToDate', files: 142, done: 0, total: 0, devices: [], conflicts: [],
    ignored: [], truncated: false,
    ...overrides,
  };
}

function device(name: string, overrides: Partial<CollectionShare['devices'][number]> = {}): CollectionShare['devices'][number] {
  return { key: name, name, state: 'active', connected: true, syncing: false, lastSyncAt: null, ...overrides };
}

function offer(deviceName: string, name: string, syncId = name): CollectionOffer {
  return { device: { key: `key-${deviceName}`, name: deviceName }, profileId: 'default', syncId, name, files: 3, bytes: 1024, suggestedDestination: `D:\\${name}` };
}

function folder(overrides: Partial<FolderSummary> = {}): FolderSummary {
  return { exists: true, empty: false, mdFiles: 87, bytes: 2048, hasGit: false, ignored: [], truncated: false, ...overrides };
}

describe('formatBytes', () => {
  it('uses the units of the language', () => {
    expect(formatBytes(512, 'en')).toBe('512 B');
    expect(formatBytes(512, 'fr')).toBe('512 o');
    expect(formatBytes(2048, 'fr')).toBe('2 Ko');
  });

  it('keeps one decimal from the megabytes up', () => {
    expect(formatBytes(3.2 * 1024 * 1024, 'en')).toBe('3.2 MB');
    expect(formatBytes(3.2 * 1024 * 1024, 'fr')).toContain('3,2');
  });

  it('never goes negative', () => {
    expect(formatBytes(-5, 'en')).toBe('0 B');
  });
});

describe('relativeAgo', () => {
  it('reads a SQLite UTC date', () => {
    expect(relativeAgo('2026-11-24 11:58:00', NOW, 'en')).toBe('2 minutes ago');
    expect(relativeAgo('2026-11-24 09:00:00', NOW, 'en')).toBe('3 hours ago');
    expect(relativeAgo('2026-11-22 12:00:00', NOW, 'fr')).toBe('avant-hier');
  });

  it('says "now" under a minute, and never for the future', () => {
    expect(relativeAgo('2026-11-24 11:59:30', NOW, 'en')).toBe('now');
    expect(relativeAgo('2026-11-24 12:05:00', NOW, 'en')).toBe('now');
  });

  it('is null without a date or for an unreadable one', () => {
    expect(relativeAgo(null, NOW, 'en')).toBeNull();
    expect(relativeAgo('yesterday', NOW, 'en')).toBeNull();
  });
});

describe('progressLabel', () => {
  it('shows done over total, capped at the total', () => {
    expect(progressLabel(37, 142)).toBe('37 / 142');
    expect(progressLabel(150, 142)).toBe('142 / 142');
    expect(progressLabel(-1, 10)).toBe('0 / 10');
  });
});

describe('shareBadge', () => {
  it('shows the progress while syncing', () => {
    const badge = shareBadge(share({ status: 'syncing', done: 37, total: 142 }), NOW, 'fr');

    expect(badge).toMatchObject({ tone: 'busy', glyph: 'sync', progress: '37 / 142', tooltip: '37 / 142 fichiers' });
  });

  it('has no progress before the first count', () => {
    const badge = shareBadge(share({ status: 'syncing' }), NOW, 'en');

    expect(badge.progress).toBeNull();
    expect(badge.tooltip).toBe('Syncing…');
  });

  it('names the devices and the last exchange when up to date', () => {
    const devices = [device('PC-Portable', { lastSyncAt: '2026-11-24 11:58:00' }), device('PC-Bureau', { lastSyncAt: '2026-11-24 10:00:00' })];
    const badge = shareBadge(share({ devices }), NOW, 'en');

    expect(badge).toMatchObject({ tone: 'ok', glyph: 'sync' });
    expect(badge.tooltip).toBe('Synced with PC-Portable, PC-Bureau, 2 minutes ago');
  });

  it('drops the time when no exchange is known', () => {
    expect(shareBadge(share({ devices: [device('PC-Portable')] }), NOW, 'en').tooltip).toBe('Synced with PC-Portable');
  });

  it('counts the conflicts', () => {
    const conflicts = [
      { path: 'a (conflit X).md', original: 'a.md', deviceName: 'X', mine: false },
      { path: 'b (conflit X).md', original: 'b.md', deviceName: 'X', mine: true },
    ];

    expect(shareBadge(share({ status: 'conflicts', conflicts }), NOW, 'fr')).toMatchObject({ tone: 'warning', glyph: 'warning', tooltip: '2 fichiers en conflit' });
    expect(shareBadge(share({ status: 'conflicts', conflicts: conflicts.slice(0, 1) }), NOW, 'fr').tooltip).toBe('1 fichier en conflit');
  });

  it('waits for the absent devices, or all of them when none is absent', () => {
    const devices = [device('PC-Portable', { connected: false }), device('PC-Bureau')];

    expect(shareBadge(share({ status: 'waiting', devices }), NOW, 'fr')).toMatchObject({ tone: 'paused', glyph: 'pause', tooltip: 'En attente de PC-Portable' });
    expect(waitingDevices(share({ devices: [device('A'), device('B')] }))).toBe('A, B');
  });

  it('says a proposal is not answered', () => {
    const devices = [device('PC-Portable', { state: 'offered' }), device('PC-Bureau')];

    expect(shareBadge(share({ status: 'offered', devices }), NOW, 'en').tooltip).toBe('Proposed to PC-Portable, waiting for an answer');
  });

  it('flags a missing folder', () => {
    expect(shareBadge(share({ status: 'missingFolder' }), NOW, 'fr')).toMatchObject({ tone: 'error', glyph: 'cross' });
  });
});

describe('lastSyncOfShare', () => {
  it('is the most recent exchange of the devices', () => {
    const devices = [device('A', { lastSyncAt: '2026-11-24 10:00:00' }), device('B', { lastSyncAt: '2026-11-24 11:00:00' }), device('C')];

    expect(lastSyncOfShare(share({ devices }))).toBe('2026-11-24 11:00:00');
    expect(lastSyncOfShare(share())).toBeNull();
  });
});

describe('conflict matching', () => {
  const conflicts = [{ path: 'docs/a (conflit PC).md', original: 'docs/a.md', deviceName: 'PC', mine: false }];
  const shares = [share({ conflicts }), share({ syncId: 's2', root: 'C:\\other' })];

  it('finds the conflict of a copy by its path, in the right collection', () => {
    expect(conflictOfFile(shares, 'C:\\notes\\wms', 'docs/a (conflit PC).md')?.original).toBe('docs/a.md');
    expect(conflictOfFile(shares, 'C:\\other', 'docs/a (conflit PC).md')).toBeNull();
  });

  it('does not take the original for a copy', () => {
    expect(conflictOfFile(shares, 'C:\\notes\\wms', 'docs/a.md')).toBeNull();
  });

  it('lists the copy paths of a collection', () => {
    expect([...conflictPaths(shares, 'C:\\notes\\wms')]).toEqual(['docs/a (conflit PC).md']);
    expect(conflictPaths(shares, 'C:\\nothing').size).toBe(0);
  });
});

describe('events applied to the sharings', () => {
  it('replaces a known sharing in place, adds an unknown one, removes with null', () => {
    const shares = [share({ syncId: 'a' }), share({ syncId: 'b' }), share({ syncId: 'c' })];
    const updated = applyShareState(shares, 'default', 'b', share({ syncId: 'b', status: 'waiting' }));

    expect(updated.map(item => `${item.syncId}:${item.status}`)).toEqual(['a:upToDate', 'b:waiting', 'c:upToDate']);
    expect(applyShareState(shares, 'default', 'd', share({ syncId: 'd' })).map(item => item.syncId)).toEqual(['a', 'b', 'c', 'd']);
    expect(applyShareState(shares, 'default', 'b', null).map(item => item.syncId)).toEqual(['a', 'c']);
  });

  it('tells apart the profiles of a same sync id', () => {
    const shares = [share({ profileId: 'p1' }), share({ profileId: 'p2' })];

    expect(applyShareState(shares, 'p2', 's1', null).map(item => item.profileId)).toEqual(['p1']);
  });

  it('shows an exchange in progress, except for a missing folder or a finished exchange', () => {
    expect(applyProgress([share()], 'default', 's1', 10, 142)[0]).toMatchObject({ status: 'syncing', done: 10, total: 142 });
    expect(applyProgress([share({ status: 'missingFolder' })], 'default', 's1', 10, 142)[0]?.status).toBe('missingFolder');
    expect(applyProgress([share({ status: 'syncing' })], 'default', 's1', 142, 142)[0]).toMatchObject({ status: 'syncing', done: 142 });
    expect(applyProgress([share()], 'default', 'other', 1, 2)[0]?.done).toBe(0);
  });

  it('shows new conflicts at once, over the quiet states only', () => {
    const conflicts = [{ path: 'a (conflit X).md', original: 'a.md', deviceName: 'X', mine: false }];

    expect(applyConflicts([share({ status: 'waiting' })], 'default', 's1', conflicts)).toMatchObject({ reload: false, shares: [{ status: 'conflicts' }] });
    expect(applyConflicts([share({ status: 'syncing' })], 'default', 's1', conflicts).shares[0]?.status).toBe('syncing');
  });

  it('asks for a reload once the conflicts are all resolved', () => {
    const result = applyConflicts([share({ status: 'conflicts', conflicts: [{ path: 'a', original: 'b', deviceName: 'X', mine: true }] })], 'default', 's1', []);

    expect(result.reload).toBe(true);
    expect(result.shares[0]?.conflicts).toEqual([]);
    expect(applyConflicts([share()], 'default', 's1', []).reload).toBe(false);
  });
});

describe('proposals', () => {
  it('sorts by device, then collection', () => {
    const sorted = sortOffers([offer('PC-B', 'zeta'), offer('PC-A', 'beta'), offer('PC-B', 'alpha')]);

    expect(sorted.map(item => `${item.device.name}/${item.name}`)).toEqual(['PC-A/beta', 'PC-B/alpha', 'PC-B/zeta']);
  });

  it('replaces the previous proposals of a device, and keeps the others', () => {
    const pending = [offer('PC-A', 'one'), offer('PC-B', 'two')];
    const merged = mergeDeviceOffers(pending, 'key-PC-A', [offer('PC-A', 'three')]);

    expect(merged.map(item => item.name)).toEqual(['three', 'two']);
    expect(mergeDeviceOffers(pending, 'key-PC-A', []).map(item => item.name)).toEqual(['two']);
  });

  it('announces only the proposals not seen yet', () => {
    const offers = [offer('PC-A', 'one'), offer('PC-A', 'two')];

    expect(newOffers(offers, new Set([offerKey(offers[0] as CollectionOffer)])).map(item => item.name)).toEqual(['two']);
  });

  it('identifies a proposal by device and sync id', () => {
    expect(offerKey(offer('PC-A', 'one', 'x'))).toBe('key-PC-A:x');
  });

  it('keeps the proposals of a profile', () => {
    const other = { ...offer('PC-A', 'one'), profileId: 'work' };

    expect(offersOfProfile([offer('PC-A', 'two'), other], 'work')).toEqual([other]);
  });
});

describe('destination folder', () => {
  it('merges an existing folder with files, receives anything else', () => {
    expect(acceptMode(folder())).toBe('merge');
    expect(acceptMode(folder({ empty: true, mdFiles: 0 }))).toBe('receive');
    expect(acceptMode(folder({ exists: false, empty: true }))).toBe('receive');
    expect(acceptMode(null)).toBe('receive');
  });

  it('warns only for a folder that holds files', () => {
    expect(needsMergeWarning(folder())).toBe(true);
    expect(needsMergeWarning(folder({ empty: true }))).toBe(false);
    expect(needsMergeWarning(folder({ exists: false }))).toBe(false);
    expect(needsMergeWarning(null)).toBe(false);
  });
});

describe('sharing assistant', () => {
  const collections: Collection[] = [
    { path: 'C:\\notes\\wms', kind: 'folder', displayName: 'WMS', sortOrder: 0 },
    { path: 'C:\\notes\\todo.md', kind: 'file', displayName: 'todo', sortOrder: 1 },
    { path: 'C:\\notes\\docs', kind: 'folder', displayName: 'Docs', sortOrder: 2 },
  ];

  it('offers the folder collections not shared yet', () => {
    expect(shareableCollections(collections, [share()]).map(item => item.displayName)).toEqual(['Docs']);
  });

  it('counts the files left out for their size', () => {
    const ignored = [
      { path: 'big.md', reason: 'tooLarge' as const },
      { path: 'bad.md', reason: 'unreadable' as const },
      { path: 'huge.md', reason: 'tooLarge' as const },
    ];

    expect(tooLargeCount(folder({ ignored }))).toBe(2);
  });

  it('finds a sharing by root and lists the ones of a profile by name', () => {
    const shares = [share({ name: 'Zed', syncId: 'z' }), share({ name: 'Alpha', syncId: 'a', root: 'C:\\a' }), share({ profileId: 'work', syncId: 'w' })];

    expect(shareOfRoot(shares, 'C:\\a')?.syncId).toBe('a');
    expect(shareOfRoot(shares, 'C:\\none')).toBeNull();
    expect(sharesOfProfile(shares, 'default').map(item => item.name)).toEqual(['Alpha', 'Zed']);
  });
});

describe('refusals of the collection sync', () => {
  it('translates the new ones', () => {
    expect(syncErrorMessage(new Error('Collection already shared'), 'en')).toBe('This collection is already synced.');
    expect(syncErrorMessage(new Error('Destination folder is not empty'), 'fr')).toContain('pas vide');
    expect(syncErrorMessage(new Error('Unknown collection share'), 'en')).toBe('This collection is no longer synced.');
  });
});
