import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config';

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'happy-dom',
      globals: true,
      setupFiles: ['./frontend/test-setup.ts'],
      include: ['frontend/__tests__/**/*.test.{ts,tsx}'],
      exclude: ['**/node_modules/**', '**/dist/**'],
      coverage: {
        provider: 'v8',
        reporter: ['text', 'html', 'lcov'],
        include: ['frontend/**/*.{ts,tsx}'],
        exclude: [
          'frontend/__tests__/**',
          'frontend/__mocks__/**',
          'frontend/main.tsx',
          'frontend/vite-env.d.ts',
        ],
        reportsDirectory: './coverage',
      },
    },
  }),
);
