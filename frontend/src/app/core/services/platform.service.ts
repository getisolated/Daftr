import { Injectable } from '@angular/core';

export type Platform = 'mac' | 'windows' | 'linux';

type NavigatorLike = Pick<Navigator, 'platform' | 'userAgent'> & {
  userAgentData?: { platform?: string };
};

/**
 * Detects the platform from navigator info, without any Tauri dependency.
 * Order: userAgentData.platform, navigator.platform, userAgent. Defaults to
 * 'windows' (historical behaviour of the app).
 * Note: WKWebView on macOS reports "MacIntel" even on Apple Silicon.
 */
export function detectPlatform(nav: NavigatorLike): Platform {
  const fromString = (value: string | undefined): Platform | null => {
    if (!value) return null;
    if (/mac/i.test(value)) return 'mac';
    if (/win/i.test(value)) return 'windows';
    if (/linux/i.test(value)) return 'linux';
    return null;
  };
  const fromUserAgent = (ua: string | undefined): Platform | null => {
    if (!ua) return null;
    if (/Mac OS X/.test(ua)) return 'mac';
    if (/Windows/.test(ua)) return 'windows';
    return null;
  };
  return (
    fromString(nav.userAgentData?.platform) ??
    fromString(nav.platform) ??
    fromUserAgent(nav.userAgent) ??
    'windows'
  );
}

/**
 * Single source of truth for the platform the app runs on.
 * `link-navigation.ts` still does its own `/mac/i.test(navigator.platform)`;
 * this service supersedes that check.
 */
@Injectable({ providedIn: 'root' })
export class PlatformService {
  readonly platform: Platform =
    typeof navigator !== 'undefined' ? detectPlatform(navigator as NavigatorLike) : 'windows';
  readonly isMac: boolean = this.platform === 'mac';
  readonly isWindows: boolean = this.platform === 'windows';
}
