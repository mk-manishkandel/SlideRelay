// Regenerates every icon from assets/logo.svg and assets/logo-mono.svg.
//
// You only need to run this after changing the logo. The generated files are
// committed, so normal builds don't need it.
//
// Requirements: `rsvg-convert` (macOS: `brew install librsvg`) and npm deps.
// Usage: npm run icons

import { execFileSync } from 'node:child_process';
import { mkdirSync, copyFileSync, rmSync } from 'node:fs';

const run = (cmd, args) => execFileSync(cmd, args, { stdio: 'inherit' });
const render = (svg, size, out) =>
  run('rsvg-convert', ['-w', String(size), '-h', String(size), svg, '-o', out]);

// 1. Office add-in icons (manifest + ribbon). Microsoft recommends these sizes.
mkdirSync('assets/icons', { recursive: true });
mkdirSync('addin/public/assets', { recursive: true });
for (const size of [16, 20, 24, 32, 40, 48, 64, 80, 96, 128, 256, 1024]) {
  const out = `assets/icons/logo-${size}.png`;
  render('assets/logo.svg', size, out);
  if (size <= 128) copyFileSync(out, `addin/public/assets/logo-${size}.png`);
}
copyFileSync('assets/logo.svg', 'addin/public/assets/logo.svg');

// 2. Tray / menu-bar icons (single-color "template" glyph).
mkdirSync('companion/src-tauri/icons', { recursive: true });
render('assets/logo-mono.svg', 32, 'companion/src-tauri/icons/tray.png');
render('assets/logo-mono.svg', 64, 'companion/src-tauri/icons/tray@2x.png');

// 3. App icons for Windows (.ico) and macOS (.icns) via the Tauri CLI.
run('npx', ['tauri', 'icon', 'assets/icons/logo-1024.png', '-o', 'companion/src-tauri/icons']);
// SlideRelay is desktop-only: drop the mobile icons the CLI also makes.
for (const dir of ['android', 'ios']) {
  rmSync(`companion/src-tauri/icons/${dir}`, { recursive: true, force: true });
}
copyFileSync('companion/src-tauri/icons/icon.ico', 'assets/icons/sliderelay.ico');
copyFileSync('companion/src-tauri/icons/icon.icns', 'assets/icons/sliderelay.icns');

console.log('Icons regenerated.');
