/** Manifest published next to the installer as a release asset (`latest.json`). */
export interface UpdateManifest {
  /** Published version, plain `x.y.z`. */
  version: string;
  /** Installer file name. */
  path: string;
  /** Full download URL of the installer. */
  url: string;
  /** Hex SHA-512 of the installer. */
  sha512: string;
}

/** Update state exposed to the renderer. */
export interface UpdateInfo {
  currentVersion: string;
  version: string;
  /** The installer of this version is downloaded and verified: installing is immediate. */
  downloaded: boolean;
}

/** Download progress of the installer. `percent` is -1 when the server gives no total size. */
export interface UpdateProgress {
  received: number;
  total: number;
  percent: number;
}

const VERSION_PATTERN = /^v?(\d+)\.(\d+)\.(\d+)/;
const SHA512_PATTERN = /^[0-9a-f]{128}$/i;
// A bare file name: the installer is written into the temp folder under this name
const INSTALLER_NAME_PATTERN = /^[\w.-]+\.exe$/i;

function parseVersion(version: string): [number, number, number] | null {
  const match = VERSION_PATTERN.exec(version);
  return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null;
}

/** True when `candidate` is strictly newer than `current`. Unparseable versions are never newer. */
export function isNewerVersion(candidate: string, current: string): boolean {
  const a = parseVersion(candidate);
  const b = parseVersion(current);
  if (!a || !b) {
    return false;
  }
  for (let i = 0; i < 3; i++) {
    if (a[i] !== b[i]) {
      return a[i] > b[i];
    }
  }
  return false;
}

/** Validates an untrusted manifest; returns null when anything is missing or suspicious. */
export function parseManifest(input: unknown): UpdateManifest | null {
  if (typeof input !== 'object' || input === null) {
    return null;
  }
  const { version, path, url, sha512 } = input as Record<string, unknown>;

  if (typeof version !== 'string' || !parseVersion(version)) return null;
  if (typeof path !== 'string' || !INSTALLER_NAME_PATTERN.test(path)) return null;
  if (typeof sha512 !== 'string' || !SHA512_PATTERN.test(sha512)) return null;
  if (typeof url !== 'string' || !url.startsWith('https://')) return null;

  return { version: version.replace(/^v/, ''), path, url, sha512: sha512.toLowerCase() };
}
