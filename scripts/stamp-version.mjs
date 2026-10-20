// Écrit une version dans tous les fichiers du dépôt qui en portent une, pour qu'ils concordent
// avec celle calculée par la release (`.github/workflows/release.yml`) :
//   - package.json, frontend/package.json et package-lock.json (racine et workspaces)
//   - src-tauri/Cargo.toml ([workspace.package]) et les crates du workspace dans Cargo.lock
//
// Usage : node scripts/stamp-version.mjs <version>
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) {
  console.error('Usage: node scripts/stamp-version.mjs <major.minor.patch>');
  process.exit(1);
}

const changed = [];
const write = (file, content) => {
  if (readFileSync(file, 'utf8') !== content) {
    writeFileSync(file, content);
    changed.push(file);
  }
};

// --- npm : les fichiers JSON sont réécrits comme npm le fait (2 espaces, saut de ligne final)
const stampJson = (file, update) => {
  const json = JSON.parse(readFileSync(file, 'utf8'));
  update(json);
  write(file, JSON.stringify(json, null, 2) + '\n');
};

const workspaces = ['', 'frontend'];
for (const dir of workspaces) {
  stampJson(join(dir, 'package.json'), pkg => { pkg.version = version; });
}
stampJson('package-lock.json', lock => {
  lock.version = version;
  for (const dir of workspaces) {
    if (lock.packages?.[dir]) lock.packages[dir].version = version;
  }
});

// --- Cargo : la version du workspace, héritée par chaque crate (`version.workspace = true`).
// Les fichiers peuvent être en CRLF sur un poste Windows : les fins de ligne sont tolérées.
const cargoToml = 'src-tauri/Cargo.toml';
const toml = readFileSync(cargoToml, 'utf8');
const stampedToml = toml.replace(/(\[workspace\.package\][^[]*?\r?\nversion = ")[^"]+(")/, `$1${version}$2`);
if (stampedToml === toml && !toml.includes(`version = "${version}"`)) {
  console.error(`${cargoToml} : [workspace.package] version introuvable`);
  process.exit(1);
}
write(cargoToml, stampedToml);

// Les crates du workspace sont les entrées de Cargo.lock sans `source` (les dépendances en ont une)
const crates = readdirSync('src-tauri/crates')
  .map(crate => readFileSync(join('src-tauri/crates', crate, 'Cargo.toml'), 'utf8').match(/^name = "([^"]+)"/m)?.[1])
  .filter(Boolean);
const cargoLock = 'src-tauri/Cargo.lock';
let lockText = readFileSync(cargoLock, 'utf8');
for (const crate of crates) {
  const entry = new RegExp(
    String.raw`(\[\[package\]\]\r?\nname = "` + crate + String.raw`"\r?\nversion = ")[^"]+("\r?\n)(?!source)`,
    'g',
  );
  lockText = lockText.replace(entry, `$1${version}$2`);
}
write(cargoLock, lockText);

console.log(changed.length ? `Version ${version} écrite dans :\n  ${changed.join('\n  ')}` : `Version ${version} déjà en place.`);
