import { defineConfig } from 'vite';
import { resolve } from 'node:path';

// Builds the companion app's windows (plain HTML + TypeScript, one page per
// window). Tauri loads them from dist/ in release builds and from the dev
// server during `npm run dev:companion`.
const page = (name: string) => resolve(import.meta.dirname, 'ui', `${name}.html`);

export default defineConfig({
  root: 'ui',
  base: './',
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: '../dist',
    emptyOutDir: true,
    target: ['es2021', 'safari15'],
    rollupOptions: {
      input: {
        main: page('index'),
        pair: page('pair'),
      },
    },
  },
});
