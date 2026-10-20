// Writes the latest.json of a release, read at releases/latest/download/latest.json:
// - by the Tauri updater: `version`, `pub_date`, `platforms` (artifact URL + minisign signature);
// - by the former Electron versions, during the transition: `version`, `path`, `url`, `sha512`
//   (computed on the Windows installer, the only artifact those versions understand).
// Each one ignores the fields of the other.
//
// Platforms, each built by its own CI job and merged here:
// - windows-x86_64: `*-setup.exe` + `.sig`
// - darwin-aarch64: `*.app.tar.gz` + `.sig`
// A missing platform is left out with a warning; no platform at all is an error.
//
// Usage: node scripts/update-manifest.mjs <artifacts dir> <tag> <owner/repo> <output file>
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';

const [artifactsDir, tag, repository, output] = process.argv.slice(2);

if (!artifactsDir || !tag || !repository || !output) {
  throw new Error('Usage: update-manifest.mjs <artifacts dir> <tag> <owner/repo> <output>');
}

// `download-artifact` with `merge-multiple` merges the artifacts into one folder, but each keeps
// its own layout (the macOS one has `dmg/` and `macos/` sub-folders): search recursively.
function listFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry =>
    entry.isDirectory() ? listFiles(join(dir, entry.name)) : [join(dir, entry.name)],
  );
}

const files = listFiles(artifactsDir);

const targets = {
  'windows-x86_64': '-setup.exe',
  'darwin-aarch64': '.app.tar.gz',
};

// `basename` and not a split on `/`: the CI also runs the tests on Windows (`\` separators)
const baseName = path => basename(path);

// GitHub turns spaces into dots in asset names: the manifest must use the published name
const urlOf = name =>
  `https://github.com/${repository}/releases/download/${tag}/${name.replaceAll(' ', '.')}`;

const platforms = {};
const found = {};

for (const [platform, suffix] of Object.entries(targets)) {
  const artifact = files.find(file => baseName(file).endsWith(suffix));
  const signatureFile = artifact && files.find(file => file === `${artifact}.sig`);

  if (!artifact || !signatureFile) {
    console.error(`Warning: no ${suffix} artifact with a .sig for ${platform}, platform left out.`);
    continue;
  }

  found[platform] = artifact;
  platforms[platform] = {
    signature: readFileSync(signatureFile, 'utf-8').trim(),
    url: urlOf(baseName(artifact)),
  };
}

if (Object.keys(platforms).length === 0) {
  throw new Error(`No updatable artifact (${Object.values(targets).join(', ')}) in ${artifactsDir}`);
}

const manifest = {
  version: tag.replace(/^v/, ''),
  pub_date: new Date().toISOString(),
  platforms,
};

const installer = found['windows-x86_64'];

if (installer) {
  const published = baseName(installer).replaceAll(' ', '.');
  manifest.path = published;
  manifest.url = urlOf(baseName(installer));
  manifest.sha512 = createHash('sha512').update(readFileSync(installer)).digest('hex');
}

writeFileSync(output, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(JSON.stringify(manifest, null, 2));
