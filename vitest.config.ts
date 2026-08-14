import { defineConfig } from 'vitest/config';

/** Configures unit tests and local coverage reports. */
export default defineConfig({
  test: {
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      reportsDirectory: 'coverage/typescript',
      include: ['lib/**/*.ts'],
      exclude: ['test/**'],
      thresholds: {
        branches: 90,
        functions: 90,
        lines: 90,
        statements: 90,
      },
    },
  },
});
