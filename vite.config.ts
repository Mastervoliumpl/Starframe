import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  build: { assetsInlineLimit: 0 },
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    watch: {
      ignored: [
        '**/src-tauri/**',
        '**/runtime/**/bin/**',
        '**/runtime/**/obj/**',
      ],
    },
  },
  test: { include: ['src/**/*.test.ts'] },
});
