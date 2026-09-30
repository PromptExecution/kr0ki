import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import { configDefaults } from 'vitest/config'
import { readFileSync } from 'fs'
import { resolve } from 'path'

// Read version from Cargo.toml
const cargoToml = readFileSync(resolve(__dirname, '../Cargo.toml'), 'utf-8')
const versionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)
const appVersion = versionMatch ? versionMatch[1] : '0.0.0'

// The kr0ki server the dev proxy forwards to: KR0KI_PUBLIC_URL from the repo-root .env
// (see .env.example), else the local default. Nothing machine-specific is hard-coded.
const kr0kiUrl =
  loadEnv(process.env.NODE_ENV || 'development', resolve(__dirname, '..'), 'KR0KI_')
    .KR0KI_PUBLIC_URL || 'http://127.0.0.1:8787'

// Relative assets work when the same build is served at /playbook/ locally and
// under /kr0ki/playbook/ on GitHub Pages.
export default defineConfig({
  base: './',
  plugins: [vue()],
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  server: {
    proxy: {
      // Proxy /api and /playbook/api requests to the kr0ki server (port 8787)
      // The kr0ki server serves /api/examples (no .json suffix), so strip it.
      '/api': {
        target: kr0kiUrl,
        changeOrigin: true,
        rewrite: (path) => path.replace(/\.json$/, ''),
      },
      '/playbook/api': {
        target: kr0kiUrl,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/playbook/, '').replace(/\.json$/, ''),
      },
    },
  },
  test: {
    environment: 'jsdom',
    // CI doesn't build the vendored @assistant-ui/vue package (a `file:`
    // dependency on the elasticdotventures/assistant-ui submodule) --
    // that requires installing the entire upstream monorepo (2,347
    // packages, ~2GB: Next.js, Nuxt, SvelteKit, docs tooling, none of
    // which kr0ki needs) just to produce one package's dist/. Excluded
    // from CI only (PR #37 review) -- this test still runs for any
    // developer who has built the vendored package locally
    // (`pnpm --dir vendor/assistant-ui/packages/vue run build`).
    exclude: process.env.CI
      ? [...configDefaults.exclude, 'src/components/__tests__/assistant-ui-vendor.test.js']
      : configDefaults.exclude,
  },
})
