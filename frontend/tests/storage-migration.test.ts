import { describe, expect, it } from 'vitest';
import { RENAMED_STORAGE_KEYS, migrateStorageKeys } from '../src/app/core/utils/storage-migration';

function fakeStorage(entries: Record<string, string>): Map<string, string> & Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> {
  const map = new Map(Object.entries(entries));
  return Object.assign(map, {
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => void map.set(key, value),
    removeItem: (key: string) => void map.delete(key),
  });
}

describe('migrateStorageKeys', () => {
  it('renames every legacy key and removes the old one', () => {
    const storage = fakeStorage({ 'evn-theme': 'light', 'evnote-session': '[1,2]', unrelated: 'kept' });

    migrateStorageKeys(storage);

    expect(storage.getItem('daftr-theme')).toBe('light');
    expect(storage.getItem('daftr-session')).toBe('[1,2]');
    expect(storage.getItem('evn-theme')).toBeNull();
    expect(storage.getItem('evnote-session')).toBeNull();
    expect(storage.getItem('unrelated')).toBe('kept');
  });

  it('never overwrites a value already saved under the new name', () => {
    const storage = fakeStorage({ 'evn-theme': 'light', 'daftr-theme': 'dark' });

    migrateStorageKeys(storage);

    expect(storage.getItem('daftr-theme')).toBe('dark');
    expect(storage.getItem('evn-theme')).toBeNull();
  });

  it('is idempotent and harmless on a fresh storage', () => {
    const storage = fakeStorage({});

    migrateStorageKeys(storage);
    migrateStorageKeys(storage);

    expect(storage.size).toBe(0);
  });

  it('matches the theme keys renamed by the inline script of index.html', () => {
    const renamed = new Map(RENAMED_STORAGE_KEYS);
    expect(renamed.get('evn-theme')).toBe('daftr-theme');
    expect(renamed.get('evn-theme-dark')).toBe('daftr-theme-dark');
    expect(renamed.get('evn-theme-light')).toBe('daftr-theme-light');
  });
});
