import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  resolve: {
    alias: {
      'electron/main': fileURLToPath(new URL('./tests/stubs/electron-main.ts', import.meta.url)),
    },
  },
  test: {
    include: ['tests/**/*.test.ts'],
    environment: 'node',
  },
});
