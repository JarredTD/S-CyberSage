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
        branches: 95,
        functions: 95,
        lines: 95,
        statements: 95,
      },
    },
  },
});
