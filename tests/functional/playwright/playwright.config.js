// @ts-check
const { defineConfig } = require('@playwright/test');

// kr0ki-server must already be running (`just run`, or the agent's own
// lifecycle) — this suite drives the real HTTP/docs surface, it does not
// spawn the server itself. See TEST-DEFINITIONS.md in this same directory
// for the human-readable spec these cases implement.
const baseURL = process.env.KR0KI_BASE_URL || 'http://localhost:8787';

module.exports = defineConfig({
  testDir: '.',
  timeout: 30_000,
  // The render backend behind KR0KI_BACKEND_URL is a single local dev
  // instance, not a pool — fullyParallel here produced transient 422s under
  // concurrent graphviz/d2/plantuml subprocess launches (verified 2026-09-29:
  // the same requests succeed 100% serially). Run serially instead of
  // papering over that with retries.
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list']],
  use: {
    baseURL,
    trace: 'retain-on-failure',
  },
});
