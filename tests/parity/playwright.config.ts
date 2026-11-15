import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: '.',
  reporter: 'list',
  timeout: 120_000,
  workers: 1,
  use: {
    baseURL: process.env.TGPREP_URL ?? 'http://localhost:5179',
    ...devices['Desktop Chrome'],
    viewport: { width: 1600, height: 1000 },
    permissions: ['clipboard-read', 'clipboard-write'],
    locale: 'en-US',
  },
})
