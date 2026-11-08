import { defineConfig } from 'vite';
import { copyFileSync, existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { homedir } from 'node:os';

// In production the add-in is served from GitHub Pages:
//   https://mk-manishkandel.github.io/SlideRelay/
// During development it runs on https://localhost:3000 (see docs/DEVELOPING.md).
// Office only loads add-ins over HTTPS, so dev uses the Office dev certificate
// created by `npx office-addin-dev-certs install`.
const devCertDir = resolve(homedir(), '.office-addin-dev-certs');
const devHttps = existsSync(resolve(devCertDir, 'localhost.crt'))
  ? {
      cert: resolve(devCertDir, 'localhost.crt'),
      key: resolve(devCertDir, 'localhost.key'),
    }
  : undefined;

// A unique id for every build. The panel compares it with version.json
// (always fetched fresh) and reloads itself if Office cached an old copy.
const BUILD_ID = process.env.GITHUB_SHA?.slice(0, 12) ?? `dev-${Date.now()}`;

export default defineConfig({
  define: { __BUILD_ID__: JSON.stringify(BUILD_ID) },
  // Relative paths, so the same build works under /SlideRelay/ and at / in dev.
  base: './',
  server: { port: 3000, strictPort: true, https: devHttps },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      input: {
        taskpane: resolve(import.meta.dirname, 'taskpane.html'),
        open: resolve(import.meta.dirname, 'open.html'),
        qr: resolve(import.meta.dirname, 'qr.html'),
      },
    },
  },
  plugins: [
    {
      // When running `npm run dev`, write manifest.dev.xml: the same manifest
      // but pointing at https://localhost:3000 with its own Id, so the dev and
      // production add-ins can be installed side by side.
      name: 'dev-manifest',
      configureServer() {
        const prod = readFileSync(resolve(import.meta.dirname, 'manifest.xml'), 'utf8');
        const dev = prod
          .replaceAll('https://mk-manishkandel.github.io/SlideRelay/', 'https://localhost:3000/')
          .replace(/<Id>[^<]+<\/Id>/, '<Id>3e1f8a52-6c1d-4f0e-9a51-0d2b7c4e9f10</Id>')
          .replace(
            '<DisplayName DefaultValue="SlideRelay"/>',
            '<DisplayName DefaultValue="SlideRelay (dev)"/>',
          )
          .replace(
            '<bt:String id="Group.Label" DefaultValue="SlideRelay"/>',
            '<bt:String id="Group.Label" DefaultValue="SlideRelay dev"/>',
          );
        writeFileSync(resolve(import.meta.dirname, 'manifest.dev.xml'), dev);
      },
    },
    {
      // Publish the manifests next to the add-in so presenters can download them.
      name: 'copy-manifests',
      closeBundle() {
        copyFileSync(
          resolve(import.meta.dirname, 'manifest.xml'),
          resolve(import.meta.dirname, 'dist/manifest.xml'),
        );
        writeFileSync(
          resolve(import.meta.dirname, 'dist/version.json'),
          JSON.stringify({ build: BUILD_ID }),
        );
      },
    },
  ],
});
