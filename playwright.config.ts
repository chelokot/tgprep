import { defineConfig, devices } from '@playwright/test'

const url = process.env.TGPREP_URL

export default defineConfig({
  testDir: 'tests/e2e',
  fullyParallel: true,
  reporter: 'list',
  use: {
    baseURL: url ?? 'http://127.0.0.1:5179',
    ...devices['Desktop Chrome'],
    viewport: { width: 1600, height: 1000 },
    permissions: ['clipboard-read', 'clipboard-write'],
    locale: 'en-US',
  },
  webServer: url
    ? undefined
    : {
        command: 'cargo run --release -p tgprep-server',
        env: { TGPREP_LISTEN: '127.0.0.1:5179', TGPREP_CLIENT_DIR: 'dist' },
        url: 'http://127.0.0.1:5179',
        reuseExistingServer: true,
        timeout: 600_000,
      },
})
