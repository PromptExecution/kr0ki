import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { configDefaults } from 'vitest/config'

// Relative assets work when the same build is served at /playbook/ locally and
// under /kr0ki/playbook/ on GitHub Pages.
export default defineConfig({
  base: './',
  plugins: [vue()],
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
