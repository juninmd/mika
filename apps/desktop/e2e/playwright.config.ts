import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './specs',

  globalSetup: './global-setup.ts',

  timeout: 120_000,
  expect: {
    timeout: 10_000,
  },
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['line'], ['html', { open: 'never' }]] : [['list']],
  use: {
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
})
