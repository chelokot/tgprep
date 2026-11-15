import { defineConfig, devices } from '@playwright/test'

const url = process.env.TGPREP_URL

export default defineConfig({
  testDir: 'tests/e2e',
  fullyParallel: true,
  reporter: 'list',
  use: {
    baseURL: url ?? 'http://localhost:5179',
    ...devices['Desktop Chrome'],
    viewport: { width: 1600, height: 1000 },
    permissions: ['clipboard-read', 'clipboard-write'],
    locale: 'en-US',
  },
  webServer: url
    ? undefined
    : {
        command: 'node node_modules/vite/bin/vite.js --port 5179 --strictPort',
        url: 'http://localhost:5179',
        reuseExistingServer: true,
      },
})
