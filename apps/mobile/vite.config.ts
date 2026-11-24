import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5373, strictPort: true, host: process.env.TAURI_DEV_HOST || false },
  // chrome83: Android 11 ships WebView 83; newer syntax (`??=`) is a SyntaxError there.
  build: { outDir: 'out', emptyOutDir: true, target: 'chrome83' },
})
