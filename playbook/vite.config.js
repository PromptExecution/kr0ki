import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Relative assets work when the same build is served at /playbook/ locally and
// under /kr0ki/playbook/ on GitHub Pages.
export default defineConfig({
  base: './',
  plugins: [vue()],
})
