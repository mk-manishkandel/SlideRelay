// Sets the app version everywhere it lives: every package.json and the
// Rust crate. The Tauri app reads its version from the root package.json.
//
//   npm run set-version -- 1.2.0
//
// The PowerPoint add-in manifest gets the same version as X.Y.Z.0, so
// PowerPoint's add-in info matches the app.
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) {
  console.error('Usage: npm run set-version -- <major.minor.patch>, e.g. 1.2.0');
  process.exit(1);
}

const root = resolve(import.meta.dirname, '..');
for (const pkg of ['', 'shared', 'addin', 'viewer', 'companion']) {
  const file = resolve(root, pkg, 'package.json');
  const json = JSON.parse(readFileSync(file, 'utf8'));
  json.version = version;
  writeFileSync(file, JSON.stringify(json, null, 2) + '\n');
}

const cargo = resolve(root, 'companion/src-tauri/Cargo.toml');
writeFileSync(
  cargo,
  readFileSync(cargo, 'utf8').replace(/^version = "[^"]+"/m, `version = "${version}"`),
);

const manifest = resolve(root, 'addin/manifest.xml');
writeFileSync(
  manifest,
  readFileSync(manifest, 'utf8').replace(
    /<Version>[^<]+<\/Version>/,
    `<Version>${version}.0</Version>`,
  ),
);

console.log(`Version set to ${version}. Next: update CHANGELOG.md, then see docs/RELEASING.md.`);
