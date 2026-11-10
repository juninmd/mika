import { defineConfig } from 'vitest/config'

// Pure logic and contracts only; UI and the mounted app are proven by the Playwright E2E.
export default defineConfig({
  test: {
    include: ['{apps,packages}/**/*.{test,spec}.ts', 'scripts/**/*.test.mts'],
    exclude: ['**/node_modules/**', '**/dist/**', '**/out/**', 'apps/desktop/e2e/**'],
    environment: 'node',
    passWithNoTests: false,
    reporters: ['dot'],
    silent: 'passed-only',
    fsModuleCache: true,
    coverage: {
      provider: 'v8',
      include: ['apps/desktop/src/**/*.ts', 'packages/*/src/**/*.ts'],
      exclude: ['**/__tests__/**', '**/*.d.ts', '**/renderer/**', '**/*.module.ts'],
      reporter: ['text-summary', 'json-summary'],
      reportsDirectory: 'coverage',
      // Ratchet: raise to the measured floor whenever coverage grows; never lower it to go green.
      // Re-measured 2026-09-22 after Stage C (Electron/Node backend deletion, Tauri-only surface):
      // statements 98.15% (was 98%), branches 95.08% (was 96%), functions 97.86% (was 98%),
      // lines 98.59% (was 99%) — the base shrank by ~564 src/main-side tests, and the surviving
      // renderer/shared/features-renderer tree has a different shape than the deleted backend.
      // Re-measured 2026-09-28: the gate had been red since 2026-09-23 (packages/llm, packages/mood
      // and the Node sidecar deleted) — the base fell to 888 statements, mostly contract/constant
      // modules only the E2E and the Rust cross-language tests exercise, and the old floor no longer
      // described any real tree. Measured on 67cd54bb: 92.95/91.7/85.54/93.83. Now, after dropping the
      // dead runtime-tokens.ts: statements 94.7%, branches 93.6%, functions 86.74%, lines 95.34%.
      thresholds: { statements: 94, branches: 93, functions: 86, lines: 95 },
    },
  },
})
