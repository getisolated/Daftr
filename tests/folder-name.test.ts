import { describe, it, expect } from 'vitest';
import { MAX_FOLDER_NAME_LENGTH, normalizeFolderName } from '../electron/database';

describe('normalizeFolderName', () => {
  it('trims the name', () => {
    expect(normalizeFolderName('  Work  ')).toBe('Work');
  });

  it('rejects empty and blank names', () => {
    expect(normalizeFolderName('')).toBeNull();
    expect(normalizeFolderName('   ')).toBeNull();
  });

  it('rejects names that are not strings (malformed IPC payload)', () => {
    expect(normalizeFolderName(42)).toBeNull();
    expect(normalizeFolderName(undefined)).toBeNull();
  });

  it('rejects names longer than the limit, accepts exactly the limit', () => {
    expect(normalizeFolderName('a'.repeat(MAX_FOLDER_NAME_LENGTH))).not.toBeNull();
    expect(normalizeFolderName('a'.repeat(MAX_FOLDER_NAME_LENGTH + 1))).toBeNull();
  });
});
