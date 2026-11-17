import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

// `fileURLToPath`, not `.pathname`: on Windows the latter gives `/D:/…`, which Node cannot open
const script = fileURLToPath(new URL('./update-manifest.mjs', import.meta.url));
const repo = 'owner/repo';

function run(dir) {
  const output = join(dir, 'latest.json');
  const result = spawnSync(process.execPath, [script, dir, 'v1.2.3', repo, output], { encoding: 'utf-8' });
  return { ...result, output };
}

function fixture({ windows = true, mac = true, linux = true } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'manifest-'));
  if (windows) {
    writeFileSync(join(dir, 'Daftr_1.2.3_x64-setup.exe'), 'exe-bytes');
    writeFileSync(join(dir, 'Daftr_1.2.3_x64-setup.exe.sig'), 'sig-win\n');
  }
  if (mac) {
    writeFileSync(join(dir, 'Daftr_1.2.3_aarch64.app.tar.gz'), 'tar-bytes');
    writeFileSync(join(dir, 'Daftr_1.2.3_aarch64.app.tar.gz.sig'), 'sig-mac\n');
  }
  if (linux) {
    writeFileSync(join(dir, 'Daftr_1.2.3_amd64.AppImage'), 'appimage-bytes');
    writeFileSync(join(dir, 'Daftr_1.2.3_amd64.AppImage.sig'), 'sig-linux\n');
    writeFileSync(join(dir, 'Daftr_1.2.3_amd64.deb'), 'deb-bytes');
    writeFileSync(join(dir, 'Daftr_1.2.3_amd64.deb.sig'), 'sig-deb\n');
  }
  return dir;
}

const base = 'https://github.com/owner/repo/releases/download/v1.2.3';

test('all platforms', () => {
  const { status, output } = run(fixture());
  assert.equal(status, 0);
  const m = JSON.parse(readFileSync(output, 'utf-8'));
  assert.equal(m.version, '1.2.3');
  assert.deepEqual(Object.keys(m.platforms).sort(), ['darwin-aarch64', 'linux-x86_64', 'linux-x86_64-deb', 'windows-x86_64']);
  assert.deepEqual(m.platforms['windows-x86_64'], { signature: 'sig-win', url: `${base}/Daftr_1.2.3_x64-setup.exe` });
  assert.deepEqual(m.platforms['darwin-aarch64'], { signature: 'sig-mac', url: `${base}/Daftr_1.2.3_aarch64.app.tar.gz` });
  assert.deepEqual(m.platforms['linux-x86_64'], { signature: 'sig-linux', url: `${base}/Daftr_1.2.3_amd64.AppImage` });
  assert.deepEqual(m.platforms['linux-x86_64-deb'], { signature: 'sig-deb', url: `${base}/Daftr_1.2.3_amd64.deb` });
  assert.equal(m.path, 'Daftr_1.2.3_x64-setup.exe');
  assert.equal(m.url, `${base}/Daftr_1.2.3_x64-setup.exe`);
  assert.equal(m.sha512, createHash('sha512').update('exe-bytes').digest('hex'));
});

test('missing macOS: warning, Windows kept', () => {
  const { status, stderr, output } = run(fixture({ mac: false, linux: false }));
  assert.equal(status, 0);
  assert.match(stderr, /darwin-aarch64/);
  const m = JSON.parse(readFileSync(output, 'utf-8'));
  assert.deepEqual(Object.keys(m.platforms), ['windows-x86_64']);
});

test('missing Windows: no legacy fields', () => {
  const { status, output } = run(fixture({ windows: false, linux: false }));
  assert.equal(status, 0);
  const m = JSON.parse(readFileSync(output, 'utf-8'));
  assert.deepEqual(Object.keys(m.platforms), ['darwin-aarch64']);
  assert.equal(m.path, undefined);
  assert.equal(m.sha512, undefined);
});

test('no platform: error', () => {
  assert.notEqual(run(fixture({ windows: false, mac: false, linux: false })).status, 0);
});

test('artifact without signature is left out', () => {
  const dir = fixture({ mac: false, linux: false });
  writeFileSync(join(dir, 'Daftr_1.2.3_aarch64.app.tar.gz'), 'x');
  const m = JSON.parse(readFileSync(run(dir).output, 'utf-8'));
  assert.deepEqual(Object.keys(m.platforms), ['windows-x86_64']);
});

test('nested folders and spaces in names', () => {
  const dir = mkdtempSync(join(tmpdir(), 'manifest-'));
  mkdirSync(join(dir, 'Windows'));
  writeFileSync(join(dir, 'Windows', 'My App_1.2.3_x64-setup.exe'), 'e');
  writeFileSync(join(dir, 'Windows', 'My App_1.2.3_x64-setup.exe.sig'), 's');
  const m = JSON.parse(readFileSync(run(dir).output, 'utf-8'));
  assert.equal(m.platforms['windows-x86_64'].url, `${base}/My.App_1.2.3_x64-setup.exe`);
});
