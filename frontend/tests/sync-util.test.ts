import { describe, it, expect } from 'vitest';
import type { Profile } from '../src/app/core/models/profile.model';
import type { PairingTicket, ProfileSync, SyncStatus, TrustedDevice } from '../src/app/core/models/sync.model';
import {
  deviceIcon, deviceIconLabel, formatCountdown, formatPairingCode, initialPairingProfiles, initialSyncLabel, isPairingCode, lastSyncAt,
  mergeCandidates, needsNewPairingCode, normalizePairingCode, offerDestinationOptions, offerTargets, pairingAddresses, parseOfferDestination,
  remainingMs, remoteChangeLabel, remoteDeletionLabel, sharedDevices, sharedProfileIds, shortDeviceId, svgDataUrl, syncErrorMessage,
  syncIndicator, syncReason,
} from '../src/app/core/utils/sync.util';

function profile(id: string, name: string | null = id): Profile {
  return { id, name, hasIcon: false, protected: false, isDefault: id === 'default' };
}

function status(state: SyncStatus['state'], devices: number | null = null, reason: string | null = null): SyncStatus {
  return { state, devices, reason };
}

function trusted(key: string, overrides: Partial<TrustedDevice> = {}): TrustedDevice {
  return {
    key, fingerprint: 'ab'.repeat(16), name: key, kind: 'unknown', origin: 'paired', addedAt: '2026-11-20 10:00:00', revokedAt: null, present: false, connected: false, profiles: [],
    ...overrides,
  };
}

describe('syncIndicator', () => {
  it('is unknown before the first state', () => {
    expect(syncIndicator(null, 'en')).toBeNull();
  });

  it('maps every state to a tone and a translated label', () => {
    expect(syncIndicator(status('disabled'), 'fr')).toMatchObject({ tone: 'off', label: 'Synchronisation désactivée', reason: null });
    expect(syncIndicator(status('paused'), 'en')).toMatchObject({ tone: 'off', label: 'Sync paused' });
    expect(syncIndicator(status('syncing'), 'fr')).toMatchObject({ tone: 'busy', label: 'Synchronisation…' });
    expect(syncIndicator(status('noDevice'), 'en')).toMatchObject({ tone: 'warning', label: 'No device on this network' });
  });

  it('counts the connected devices when up to date', () => {
    expect(syncIndicator(status('upToDate', 1), 'en')?.label).toBe('Up to date (1 device)');
    expect(syncIndicator(status('upToDate', 3), 'fr')).toMatchObject({ tone: 'ok', label: 'À jour (3 appareils)' });
  });

  it('gives the cause of an error for the tooltip', () => {
    const shown = syncIndicator(status('error', null, 'listen_failed'), 'fr');

    expect(shown).toMatchObject({ tone: 'error', label: 'Erreur de synchronisation' });
    expect(shown?.reason).toContain('pare-feu');
    expect(syncReason('discovery_failed', 'en')).toContain('discovered');
    expect(syncReason('weird', 'en')).toBe('Unexpected failure (weird).');
  });
});

describe('syncErrorMessage', () => {
  it('translates the stable keys of the engine', () => {
    expect(syncErrorMessage(new Error('wrong_code'), 'fr')).toContain('Code incorrect');
    expect(syncErrorMessage(new Error('listen_failed'), 'en')).toContain('firewall');
    expect(syncErrorMessage(new Error('not_local'), 'en')).toBe('Only addresses of the local network can be used.');
  });

  it('translates the refusals of the input checks', () => {
    expect(syncErrorMessage(new Error('invalid pairing code'), 'en')).toBe('Enter the 6-digit code.');
    expect(syncErrorMessage(new Error('profile already synchronized'), 'fr')).toBe('Ce profil est déjà synchronisé.');
  });

  it('keeps an unknown message after a generic sentence', () => {
    expect(syncErrorMessage(new Error('disk full'), 'en')).toBe('The sync failed: disk full');
    expect(syncErrorMessage('raw', 'fr')).toBe('La synchronisation a échoué : raw');
  });
});

describe('pairing countdown and code', () => {
  it('counts down to the end of the mode, never below zero', () => {
    expect(remainingMs(10_000, 4_000)).toBe(6_000);
    expect(remainingMs(10_000, 12_000)).toBe(0);
  });

  it('shows minutes and seconds, rounding the seconds up', () => {
    expect(formatCountdown(300_000)).toBe('5:00');
    expect(formatCountdown(299_001)).toBe('5:00');
    expect(formatCountdown(299_000)).toBe('4:59');
    expect(formatCountdown(9_500)).toBe('0:10');
    expect(formatCountdown(1)).toBe('0:01');
    expect(formatCountdown(0)).toBe('0:00');
    expect(formatCountdown(-5)).toBe('0:00');
  });

  it('keeps the digits of a typed code, six at most', () => {
    expect(normalizePairingCode(' 123-456 ')).toBe('123456');
    expect(normalizePairingCode('1234567')).toBe('123456');
    expect(isPairingCode('123456')).toBe(true);
    expect(isPairingCode('12345')).toBe(false);
    expect(isPairingCode('12a456')).toBe(false);
    expect(formatPairingCode('123456')).toBe('123 456');
  });

  it('reads the addresses of the QR content', () => {
    expect(pairingAddresses('daftr-pair:1:192.168.1.20:47820,[fe80::1]:47820:abcdef:123456')).toBe('192.168.1.20:47820, [fe80::1]:47820');
    expect(pairingAddresses('something else')).toBe('');
  });

  it('shows the QR SVG as an image URL', () => {
    const url = svgDataUrl('<svg xmlns="http://www.w3.org/2000/svg"><rect/></svg>');

    expect(url.startsWith('data:image/svg+xml;charset=utf-8,')).toBe(true);
    expect(decodeURIComponent(url.slice(url.indexOf(',') + 1))).toBe('<svg xmlns="http://www.w3.org/2000/svg"><rect/></svg>');
  });

  it('checks the active profile when "Link a device" opens', () => {
    expect(initialPairingProfiles([profile('default'), profile('work')], 'work')).toEqual(['work']);
    expect(initialPairingProfiles([profile('default')], 'gone')).toEqual([]);
  });
});

describe('destination of a proposed profile', () => {
  const profiles = [profile('default', null), profile('work', 'Work'), profile('home', 'Home')];
  const offer = { spaceId: 'a'.repeat(64), name: 'Notes', noteCount: 12 };
  const name = (candidate: Profile): string => candidate.name ?? 'My notes';

  it('only proposes to merge into the profiles not synchronized yet', () => {
    expect(mergeCandidates(profiles, ['work']).map(candidate => candidate.id)).toEqual(['default', 'home']);

    const options = offerDestinationOptions(offer, profiles, ['work'], name, 'en');

    expect(options).toEqual([
      { value: 'new', label: 'New profile “Notes”' },
      { value: 'merge:default', label: 'Merge with “My notes”' },
      { value: 'merge:home', label: 'Merge with “Home”' },
      { value: 'ignore', label: 'Ignore' },
    ]);
  });

  it('offers only a new profile or nothing when every profile is synchronized', () => {
    const options = offerDestinationOptions(offer, profiles, ['default', 'work', 'home'], name, 'fr');

    expect(options.map(option => option.value)).toEqual(['new', 'ignore']);
    expect(options[0]?.label).toBe('Nouveau profil « Notes »');
  });

  it('turns the chosen value back into the answer', () => {
    expect(parseOfferDestination('new')).toEqual({ choice: 'new', profileId: null });
    expect(parseOfferDestination('ignore')).toEqual({ choice: 'ignore', profileId: null });
    expect(parseOfferDestination('merge:home')).toEqual({ choice: 'merge', profileId: 'home' });
  });

  it('shows the progress of the initial sync', () => {
    expect(initialSyncLabel({ profileId: 'work', done: 120, total: 450 }, 'fr')).toBe('Synchronisation initiale : 120 / 450 notes');
    expect(initialSyncLabel({ profileId: 'work', done: 451, total: 450 }, 'en')).toBe('Initial sync: 450 / 450 notes');
    expect(initialSyncLabel(null, 'en')).toBe('Initial sync: waiting for the notes…');
  });
});

describe('devices of a profile', () => {
  const sync: ProfileSync = {
    profileId: 'work',
    spaceId: 'b'.repeat(64),
    devices: [
      { key: 'self', fingerprint: '1'.repeat(32), name: 'Desktop', kind: 'desktop', isSelf: true, revoked: false },
      { key: 'laptop', fingerprint: '2'.repeat(32), name: 'Laptop', kind: 'laptop', isSelf: false, revoked: false },
      { key: 'old', fingerprint: '3'.repeat(32), name: 'Old PC', kind: 'unknown', isSelf: false, revoked: true },
    ],
  };

  it('lists the other devices still sharing the profile', () => {
    expect(sharedDevices(sync).map(device => device.name)).toEqual(['Laptop']);
    expect(sharedDevices(null)).toEqual([]);
  });

  it('proposes the profile to the trusted devices not sharing it yet', () => {
    const devices = [trusted('laptop'), trusted('tablet'), trusted('old'), trusted('gone', { revokedAt: '2026-11-21 08:00:00' })];

    expect(offerTargets(devices, sync).map(device => device.key)).toEqual(['tablet', 'old']);
    expect(offerTargets(devices, null).map(device => device.key)).toEqual(['laptop', 'tablet', 'old']);
  });

  it('tells the last exchange and the profiles still shared with a device', () => {
    const device = trusted('laptop', {
      profiles: [
        { profileId: 'work', revoked: false, lastSyncAt: '2026-11-22 09:00:00' },
        { profileId: 'home', revoked: true, lastSyncAt: '2026-11-23 07:30:00' },
        { profileId: 'default', revoked: false, lastSyncAt: null },
      ],
    });

    expect(lastSyncAt(device)).toBe('2026-11-23 07:30:00');
    expect(lastSyncAt(trusted('new'))).toBeNull();
    expect(sharedProfileIds(device)).toEqual(['work', 'default']);
  });
});

describe('editor labels', () => {
  it('names the device when the event does', () => {
    expect(remoteChangeLabel('Laptop', 'fr')).toBe('Modifié depuis Laptop');
    expect(remoteChangeLabel(null, 'fr')).toBe('Modifié depuis un autre appareil');
    expect(remoteDeletionLabel('Laptop', 'en')).toBe('This note was deleted on Laptop: it is read-only here.');
    expect(remoteDeletionLabel(null, 'en')).toContain('another device');
  });
});

describe('identity of a device', () => {
  it('shows the start of the fingerprint, in two groups of four', () => {
    expect(shortDeviceId('4f2a9c1b77e0d3a5c6b8e9f001122334')).toBe('4F2A-9C1B');
  });

  it('keeps a short fingerprint as it is', () => {
    expect(shortDeviceId('4f2a')).toBe('4F2A');
    expect(shortDeviceId('')).toBe('');
  });

  it('draws each kind of device, the generic device when the kind is unknown', () => {
    expect(deviceIcon('laptop')).toBe('laptop');
    expect(deviceIcon('desktop')).toBe('desktop');
    expect(deviceIcon('unknown')).toBe('device');
  });

  it('names the icons for screen readers', () => {
    expect(deviceIconLabel('laptop', 'fr')).toBe('Ordinateur portable');
    expect(deviceIconLabel('desktop', 'en')).toBe('Desktop computer');
    expect(deviceIconLabel('device', 'en')).toBe('Device');
  });
});

describe('needsNewPairingCode', () => {
  const ticket: PairingTicket = { code: '123456', uri: 'daftr-pair:1:10.0.0.2:47820:ab:123456', qrSvg: '<svg/>', expiresAt: 300_000 };

  it('asks for a code when none is open or the last one ran out', () => {
    expect(needsNewPairingCode(null, 0, false)).toBe(true);
    expect(needsNewPairingCode(ticket, 0, false)).toBe(true);
  });

  it('keeps a code still valid', () => {
    expect(needsNewPairingCode(ticket, 1000, false)).toBe(false);
  });

  it('waits while a code is on its way, after a failure, or while another code is typed', () => {
    expect(needsNewPairingCode(null, 0, true)).toBe(false);
    expect(needsNewPairingCode(ticket, 0, true)).toBe(false);
  });
});
