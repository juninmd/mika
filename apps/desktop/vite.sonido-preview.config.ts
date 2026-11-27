import { resolve } from 'node:path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

/**
 * Harness da comparação antes × depois da migração para o Sonido.
 * Roda de `apps/desktop` (é de onde vêm React, Vite e o Tailwind), mas serve `docs/sonido-migration`.
 *
 *   pnpm --filter @mika/desktop exec vite -c vite.sonido-preview.config.ts
 *   http://localhost:5302/preview.html
 */
const REPO = resolve(__dirname, '../..')
const DOCS = resolve(REPO, 'docs/sonido-migration')
const SONIDO = resolve(REPO, 'packages/sdk-ui/node_modules/@juninmd/sonido')

export default defineConfig({
  plugins: [react(), tailwindcss()],
  root: DOCS,
  server: {
    port: 5302,
    strictPort: true,
    fs: { allow: [REPO] },
  },
  resolve: {
    dedupe: ['react', 'react-dom'],
    alias: [
      // Explicit files: the docs root has no node_modules of its own.
      { find: '@juninmd/sonido/styles.css', replacement: resolve(SONIDO, 'dist/sonido.css') },
      { find: '@juninmd/sonido', replacement: resolve(SONIDO, 'dist/index.js') },
      { find: 'react-dom', replacement: resolve(__dirname, 'node_modules/react-dom') },
      { find: 'react', replacement: resolve(__dirname, 'node_modules/react') },
      { find: 'framer-motion', replacement: resolve(__dirname, 'node_modules/framer-motion') },
      { find: 'lucide-react', replacement: resolve(__dirname, 'node_modules/lucide-react') },
      { find: '@mika/sdk-ui/settings', replacement: resolve(REPO, 'packages/sdk-ui/src/settings.ts') },
      { find: '@mika/sdk-ui', replacement: resolve(REPO, 'packages/sdk-ui/src/index.tsx') },
      { find: '@mika/sdk', replacement: resolve(REPO, 'packages/sdk/src') },
    ],
  },
})
