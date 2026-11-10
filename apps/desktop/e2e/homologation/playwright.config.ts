import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: '.',

  timeout: 15 * 60_000,
  expect: { timeout: 20_000 },
  workers: 1,
  retries: 0,
  reporter: [['list'], ['json', { outputFile: 'report/results.json' }]],
  outputDir: 'report/artifacts',
  use: {
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'off',
  },
})
