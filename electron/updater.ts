import { app } from 'electron/main';
import { spawn } from 'child_process';
import { createHash } from 'crypto';
import { writeFile } from 'fs/promises';
import * as path from 'path';
import { broadcastToWindows } from './overlay';
import { getSettings } from './settings';
import { isNewerVersion, parseManifest, UpdateInfo, UpdateManifest, UpdateProgress } from './update-manifest';

// The manifest is a release asset at a stable URL: `latest` always follows the newest published release.
// Overridable so a fork (or a test repository) can serve its own updates.
const UPDATE_REPOSITORY = process.env['UPDATE_REPOSITORY'] || 'getisolated/Everything-Note';
const MANIFEST_URL = `https://github.com/${UPDATE_REPOSITORY}/releases/latest/download/latest.json`;

const CHECK_INTERVAL_MS = 60 * 60 * 1000;
// A failed check (offline, GitHub down) is retried sooner than the normal interval
const RETRY_DELAY_MS = 10 * 60 * 1000;
const MANIFEST_TIMEOUT_MS = 15_000;

let manifest: UpdateManifest | null = null;
let info: UpdateInfo | null = null;
let checkTimer: NodeJS.Timeout | null = null;
let started = false;
// Set once the installer is launched: the app is closing and must not install twice
let installing = false;

/** Verified installer already on disk; reused while the published version does not change. */
let downloaded: { version: string; path: string } | null = null;
let downloading: Promise<string> | null = null;

/** Only the NSIS installer (Windows) can replace the app; dev builds have no matching release. */
function updatesSupported(): boolean {
  return app.isPackaged && process.platform === 'win32';
}

export function getUpdateInfo(): UpdateInfo | null {
  return info;
}

/** Checks for an update at startup, then every hour. */
export function startUpdater(): void {
  if (started) return;
  if (!updatesSupported()) {
    console.warn('[Updater] Updates are only checked in the packaged Windows app');
    return;
  }
  started = true;

  // Auto mode: an update downloaded but never applied is installed when the app closes, without relaunching
  app.on('before-quit', () => {
    if (!installing && getSettings().autoUpdate && downloaded) {
      launchInstallerOnExit(downloaded.path, false);
    }
  });

  void runScheduledCheck();
}

/** Called when the user toggles "automatic updates": an update already found is taken over right away. */
export function onAutoUpdateChanged(): void {
  if (getSettings().autoUpdate && info) {
    void prepareDownload();
  }
}

/** Downloads (if needed), verifies and installs the update, then restarts the app. */
export async function applyUpdate(): Promise<void> {
  if (!manifest || !info) {
    throw new Error('No update available');
  }
  const installerPath = await downloadOnce(manifest);
  launchInstallerOnExit(installerPath, true);
}

async function runScheduledCheck(): Promise<void> {
  let next = CHECK_INTERVAL_MS;
  try {
    await checkForUpdate();
  } catch (err) {
    next = RETRY_DELAY_MS;
    console.warn('[Updater] Check failed:', err instanceof Error ? err.message : err);
  }
  checkTimer = setTimeout(() => void runScheduledCheck(), next);
}

async function checkForUpdate(): Promise<void> {
  const response = await fetch(MANIFEST_URL, { redirect: 'follow', signal: AbortSignal.timeout(MANIFEST_TIMEOUT_MS) });
  if (!response.ok) {
    throw new Error(`Manifest request failed: ${response.status} ${response.statusText}`);
  }
  const latest = parseManifest(await response.json());
  if (!latest) {
    throw new Error(`Malformed update manifest at ${MANIFEST_URL}`);
  }

  const current = app.getVersion();
  if (!isNewerVersion(latest.version, current)) {
    return;
  }

  const isNewVersion = info?.version !== latest.version;
  manifest = latest;
  info = { currentVersion: current, version: latest.version, downloaded: downloaded?.version === latest.version };
  if (isNewVersion) {
    broadcastToWindows('update:available', info);
  }

  if (getSettings().autoUpdate) {
    void prepareDownload();
  }
}

/** Auto mode: fetches the installer in the background. A failure is retried at the next check. */
async function prepareDownload(): Promise<void> {
  if (!manifest) return;
  try {
    await downloadOnce(manifest);
  } catch (err) {
    console.warn('[Updater] Background download failed:', err instanceof Error ? err.message : err);
  }
}

/** One download per version, even when auto mode and the user's click ask for it at the same time. */
function downloadOnce(target: UpdateManifest): Promise<string> {
  if (downloaded?.version === target.version) {
    return Promise.resolve(downloaded.path);
  }
  if (!downloading) {
    downloading = download(target)
      .then((installerPath) => {
        downloaded = { version: target.version, path: installerPath };
        if (info?.version === target.version) {
          info = { ...info, downloaded: true };
          broadcastToWindows('update:downloaded', info);
        }
        return installerPath;
      }, (err: unknown) => {
        broadcastToWindows('update:download-failed', err instanceof Error ? err.message : String(err));
        throw err;
      })
      .finally(() => { downloading = null; });
  }
  return downloading;
}

async function download(target: UpdateManifest): Promise<string> {
  const response = await fetch(target.url, { redirect: 'follow' });
  if (!response.ok || !response.body) {
    throw new Error(`Download failed: ${response.status} ${response.statusText}`);
  }

  const total = Number(response.headers.get('content-length') ?? 0);
  const chunks: Uint8Array[] = [];
  let received = 0;
  let lastPercent = -1;

  for await (const chunk of response.body) {
    chunks.push(chunk);
    received += chunk.byteLength;
    const percent = total > 0 ? Math.floor((received / total) * 100) : -1;
    // One event per percent point: enough for a progress label without flooding the IPC
    if (percent !== lastPercent) {
      lastPercent = percent;
      broadcastToWindows('update:progress', { received, total, percent } satisfies UpdateProgress);
    }
  }

  const buffer = Buffer.concat(chunks);
  // The installer runs with the user's rights: a truncated or swapped file must never be executed
  if (createHash('sha512').update(buffer).digest('hex') !== target.sha512) {
    throw new Error('Downloaded installer is corrupted (SHA-512 mismatch)');
  }

  const installerPath = path.join(app.getPath('temp'), target.path);
  await writeFile(installerPath, buffer);
  return installerPath;
}

/**
 * Quits the app, then runs the installer once the process is really gone.
 * Spawning before the quit completes races with the files still being in use.
 */
function launchInstallerOnExit(installerPath: string, relaunch: boolean): void {
  if (installing) return;
  installing = true;

  // Silent: the NSIS wizard is for the first install only. `--updated` marks it as an update,
  // `--force-run` restarts the app once installed.
  const args = ['/S', '--updated', ...(relaunch ? ['--force-run'] : [])];

  app.once('will-quit', () => {
    // Detached + unref so the installer outlives the app it replaces
    spawn(installerPath, args, { detached: true, stdio: 'ignore' }).unref();
  });

  // Already quitting (auto mode, `before-quit` handler): quitting again would be a no-op
  if (relaunch) {
    app.quit();
  }
}

export function stopUpdater(): void {
  if (checkTimer) {
    clearTimeout(checkTimer);
    checkTimer = null;
  }
}
