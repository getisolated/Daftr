// Writes the latest.json of a release, read at releases/latest/download/latest.json:
// - by the Tauri updater: `version`, `pub_date`, `platforms` (installer URL + minisign signature);
// - by the former Electron versions, during the transition: `version`, `path`, `url`, `sha512`.
// Each one ignores the fields of the other.
//
// Usage: node scripts/update-manifest.mjs <nsis bundle dir> <tag> <owner/repo> <output file>
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [bundleDir, tag, repository, output] = process.argv.slice(2);

if (!bundleDir || !tag || !repository || !output) {
  throw new Error('Usage: update-manifest.mjs <bundle dir> <tag> <owner/repo> <output>');
}

const installer = readdirSync(bundleDir).find(file => file.endsWith('-setup.exe'));

if (!installer) {
  throw new Error(`No NSIS installer in ${bundleDir}`);
}

// GitHub turns spaces into dots in asset names: the manifest must use the published name
const published = installer.replaceAll(' ', '.');
const url = `https://github.com/${repository}/releases/download/${tag}/${published}`;
const signature = readFileSync(join(bundleDir, `${installer}.sig`), 'utf-8').trim();
const sha512 = createHash('sha512').update(readFileSync(join(bundleDir, installer))).digest('hex');

const manifest = {
  version: tag.replace(/^v/, ''),
  pub_date: new Date().toISOString(),
  platforms: { 'windows-x86_64': { signature, url } },
  path: published,
  url,
  sha512,
};

writeFileSync(output, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(JSON.stringify(manifest, null, 2));
