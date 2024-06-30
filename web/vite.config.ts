import path from 'path'
import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: {
      '@lib': path.resolve(__dirname, './src/lib')
    }
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3000'
    }
  }
});
