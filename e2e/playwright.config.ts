import { defineConfig } from '@playwright/test';

// The recorder e2e suite drives the REAL shipped binary (videoeditor record)
// against a throwaway copy of the hello-bench example episode, in a real
// Chromium with a fake mic (a generated tone). Run via `just e2e` — the nix
// devshell provides playwright + pinned browsers (PLAYWRIGHT_BROWSERS_PATH).

export default defineConfig({
  testDir: './tests',
  timeout: 60_000,
  // one worker: every test shares the one recorder server + episode dir
  workers: 1,
  globalSetup: './global-setup',
  globalTeardown: './global-teardown',
  use: {
    baseURL: 'http://127.0.0.1:4901',
    // nix's playwright-driver bundle ships the FULL chromium (no separate
    // headless shell); 'chromium' channel = new headless mode on that build
    channel: 'chromium',
    launchOptions: {
      args: [
        '--use-fake-ui-for-media-stream',
        '--use-fake-device-for-media-stream',
        '--autoplay-policy=no-user-gesture-required',
      ],
    },
    permissions: ['microphone'],
  },
});
