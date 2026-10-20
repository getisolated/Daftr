/**
 * localStorage keys renamed when the app became Daftr (old name → new name). The inline script of
 * index.html renames the three theme keys itself, before the first paint: keep both lists in sync.
 */
export const RENAMED_STORAGE_KEYS: readonly (readonly [string, string])[] = [
  ['evn-theme', 'daftr-theme'],
  ['evn-theme-dark', 'daftr-theme-dark'],
  ['evn-theme-light', 'daftr-theme-light'],
  ['evn-recent-colors', 'daftr-recent-colors'],
  ['evnote-wide-mode', 'daftr-wide-mode'],
  ['evnote-home-view', 'daftr-home-view'],
  ['evnote-sidebar', 'daftr-sidebar'],
  ['evnote-session', 'daftr-session'],
  ['evnote-backlinks-open', 'daftr-backlinks-open'],
  ['evnote-collections-expanded', 'daftr-collections-expanded'],
  ['evnote-collections-open', 'daftr-collections-open'],
];

type KeyValueStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

/**
 * Copies every old key to its new name (unless the new one is already set) and drops the old one.
 * Idempotent: a second run finds nothing to do.
 */
export function migrateStorageKeys(storage: KeyValueStorage, renamed = RENAMED_STORAGE_KEYS): void {
  for (const [from, to] of renamed) {
    const legacy = storage.getItem(from);
    if (legacy === null) continue;
    if (storage.getItem(to) === null) {
      storage.setItem(to, legacy);
    }
    storage.removeItem(from);
  }
}
