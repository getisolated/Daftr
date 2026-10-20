import { describe, it, expect } from 'vitest';
import { detectPlatform } from '../src/app/core/services/platform.service';

describe('detectPlatform', () => {
  it('detects mac from MacIntel (WKWebView, also on Apple Silicon)', () => {
    expect(detectPlatform({ platform: 'MacIntel', userAgent: '' })).toBe('mac');
  });

  it('detects windows from Win32', () => {
    expect(detectPlatform({ platform: 'Win32', userAgent: '' })).toBe('windows');
  });

  it('detects linux from Linux x86_64', () => {
    expect(detectPlatform({ platform: 'Linux x86_64', userAgent: '' })).toBe('linux');
  });

  it('prefers userAgentData.platform over navigator.platform', () => {
    expect(
      detectPlatform({ platform: 'Win32', userAgent: '', userAgentData: { platform: 'macOS' } }),
    ).toBe('mac');
  });

  it('falls back to userAgent', () => {
    expect(detectPlatform({ platform: '', userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)' })).toBe('mac');
    expect(detectPlatform({ platform: '', userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64)' })).toBe('windows');
  });

  it('defaults to windows', () => {
    expect(detectPlatform({ platform: '', userAgent: '' })).toBe('windows');
  });
});
