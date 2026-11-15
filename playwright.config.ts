import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: 'tests/e2e',
  fullyParallel: true,
  reporter: 'list',
  use: {
    baseURL: 'http://localhost:5179',
    ...devices['Desktop Chrome'],
    viewport: { width: 1600, height: 1000 },
    permissions: ['clipboard-read', 'clipboard-write'],
    locale: 'en-US',
  },
  webServer: {
    command: 'node node_modules/vite/bin/vite.js --port 5179 --strictPort',
    url: 'http://localhost:5179',
    reuseExistingServer: true,
  },
})
