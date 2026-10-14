import { describe, it, expect } from 'vitest';
import { isNewerVersion, parseManifest } from '../electron/update-manifest';

const SHA = 'a'.repeat(128);

function manifest(overrides: Record<string, unknown> = {}): unknown {
  return {
    version: '1.2.3',
    path: 'EverythingNote-Setup-1.2.3.exe',
    url: 'https://github.com/o/r/releases/download/v1.2.3/EverythingNote-Setup-1.2.3.exe',
    sha512: SHA,
    ...overrides,
  };
}

describe('isNewerVersion', () => {
  it('compares each segment numerically', () => {
    expect(isNewerVersion('0.0.9', '0.0.8')).toBe(true);
    expect(isNewerVersion('0.10.0', '0.9.9')).toBe(true);
    expect(isNewerVersion('2.0.0', '1.99.99')).toBe(true);
  });

  it('is false for equal or older versions', () => {
    expect(isNewerVersion('1.0.0', '1.0.0')).toBe(false);
    expect(isNewerVersion('1.0.0', '1.0.1')).toBe(false);
    expect(isNewerVersion('0.9.0', '1.0.0')).toBe(false);
  });

  it('accepts a leading v and ignores suffixes', () => {
    expect(isNewerVersion('v1.0.1', '1.0.0')).toBe(true);
    expect(isNewerVersion('1.0.0-beta', '1.0.0')).toBe(false);
  });

  it('is false when a version cannot be parsed', () => {
    expect(isNewerVersion('latest', '1.0.0')).toBe(false);
    expect(isNewerVersion('1.0.0', '')).toBe(false);
  });
});

describe('parseManifest', () => {
  it('accepts a valid manifest and normalizes it', () => {
    expect(parseManifest(manifest({ version: 'v1.2.3', sha512: SHA.toUpperCase() }))).toEqual({
      version: '1.2.3',
      path: 'EverythingNote-Setup-1.2.3.exe',
      url: 'https://github.com/o/r/releases/download/v1.2.3/EverythingNote-Setup-1.2.3.exe',
      sha512: SHA,
    });
  });

  it('rejects non-objects and missing fields', () => {
    expect(parseManifest(null)).toBeNull();
    expect(parseManifest('x')).toBeNull();
    expect(parseManifest({})).toBeNull();
  });

  it('rejects an installer name that could escape the temp folder', () => {
    expect(parseManifest(manifest({ path: '..\\evil.exe' }))).toBeNull();
    expect(parseManifest(manifest({ path: 'a/b.exe' }))).toBeNull();
    expect(parseManifest(manifest({ path: 'setup.msi' }))).toBeNull();
  });

  it('rejects a non-https URL', () => {
    expect(parseManifest(manifest({ url: 'http://example.com/a.exe' }))).toBeNull();
    expect(parseManifest(manifest({ url: 'file:///a.exe' }))).toBeNull();
  });

  it('rejects a malformed hash or version', () => {
    expect(parseManifest(manifest({ sha512: 'abc' }))).toBeNull();
    expect(parseManifest(manifest({ version: 'next' }))).toBeNull();
  });
});
