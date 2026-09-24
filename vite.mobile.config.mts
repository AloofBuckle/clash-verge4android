import path from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import svgr from 'vite-plugin-svgr'

export default defineConfig({
  root: 'src/mobile',
  plugins: [svgr(), react()],
  server: {
    port: 1420,
    strictPort: true,
    host: process.env.TAURI_DEV_HOST || '127.0.0.1',
  },
  build: {
    outDir: '../../dist-mobile',
    emptyOutDir: true,
    target: 'chrome85',
  },
  resolve: { alias: { '@': path.resolve('./src') } },
})
