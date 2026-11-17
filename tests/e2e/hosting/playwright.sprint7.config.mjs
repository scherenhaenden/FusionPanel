import { fileURLToPath } from 'node:url';

export default {
  testDir: fileURLToPath(new URL('.', import.meta.url)),
  testMatch: ['sprint7.spec.mjs'],
  fullyParallel: false,
  workers: 1,
  reporter: 'line',
  use: {
    baseURL: process.env.FUSIONPANEL_FRONTEND_URL,
    browserName: 'chromium',
    trace: 'on-first-retry',
  },
};
