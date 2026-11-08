import { defineConfig } from 'vite';

// The viewer is served by the companion app from any path (/<code>), so
// assets use absolute /assets/... URLs. Older phone browsers are supported.
export default defineConfig({
  base: '/',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: ['es2019', 'safari12', 'chrome70'],
    assetsInlineLimit: 0,
  },
});
