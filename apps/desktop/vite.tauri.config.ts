import { existsSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { DEV_CSP_META, PROD_CSP_META, withTauriIpc } from './src/shared/csp-policy'

/**
 * Build do renderer para o alvo Tauri.
 *
 * Espelha a seção `renderer` de electron.vite.config.ts. As diferenças que importam:
 * o alias de `electron-runtime` e a injeção da ponte antes do app.
 */

const SOURCE_HTML = resolve(__dirname, 'src/renderer/index.html')
const TAURI_HTML = resolve(__dirname, 'src/renderer/index.tauri.html')

/**
 * Gera o HTML de entrada do alvo Tauri a partir do `index.html` original, injetando a ponte
 * como primeiro módulo. É artefato, não fonte: derivar em vez de manter uma cópia evita que
 * as duas páginas divirjam em silêncio quando alguém mexer só numa.
 *
 * Feito aqui, na carga da config, e não num hook de plugin: o `transform` do rolldown-vite
 * não disparou no `main.tsx`, e uma injeção que falha calada devolve uma janela transparente
 * sem pintar nada e sem erro — já custou um ciclo inteiro de diagnóstico.
 */
function writeTauriHtml(): string {
  const source = readFileSync(SOURCE_HTML, 'utf8')
  const mainTag = /<script\b[^>]*src=["']\.\/src\/main\.tsx["'][^>]*><\/script>/

  if (!mainTag.test(source)) {
    throw new Error(`vite.tauri.config: tag de entrada não encontrada em ${SOURCE_HTML}`)
  }

  // SUBSTITUI a tag do app: a entrada única importa a ponte e depois o `main`. Deixar as duas
  // tags fez o rolldown emitir uma entrada só e a ponte sumiu do bundle sem erro.
  const bridged = source.replace(mainTag, '<script type="module" src="./src/tauri-bridge-entry.ts"></script>')
  writeFileSync(TAURI_HTML, bridged)
  return TAURI_HTML
}

/**
 * Faz o renderer falar com Tauri em vez de Electron. Duas coisas, ambas no grafo de módulos
 * e não em string de HTML — a versão anterior mexia no `index.html` e no alias, passava no
 * build e falhava em runtime sem erro nenhum, com a janela transparente sem pintar nada.
 *
 * 1. `resolveId`: qualquer import de `shared/electron-runtime` vira o runtime Tauri. Alias de
 *    Vite não serve aqui: ele casa contra o especificador escrito (`../shared/electron-runtime`),
 *    e uma chave de caminho absoluto jamais casa com um especificador relativo.
 * 2. `transform`: prefixa o import da ponte no `main.tsx`, para `window.electronAPI` existir
 *    antes de qualquer componente montar.
 */
function tauriBridgePlugin() {
  const outDir = resolve(__dirname, 'out/renderer')

  return {
    name: 'vite-plugin-tauri-bridge',

    /**
     * Em dev o Vite serve `index.html` da raiz — o original, sem a ponte. Sem este desvio
     * `pnpm dev` sobe uma janela sem `window.electronAPI`, que é o mesmo sintoma mudo do build.
     */
    configureServer(server: { middlewares: { use: (fn: unknown) => void } }) {
      server.middlewares.use((req: { url?: string }, _res: unknown, next: () => void) => {
        if (req.url === '/' || req.url?.startsWith('/index.html')) {
          req.url = '/index.tauri.html'
        }
        next()
      })
    },

    closeBundle() {
      const built = resolve(outDir, 'index.tauri.html')
      const target = resolve(outDir, 'index.html')
      if (existsSync(built)) {
        renameSync(built, target)
      }

      // Gate: a ponte some em silêncio e o sintoma é uma janela transparente sem erro
      // nenhum. Verificar a SAÍDA, não a intenção — o build anterior "injetou" e o
      // bundle saiu sem uma linha da ponte.
      const html = readFileSync(target, 'utf8')
      const entry = html.match(/<script[^>]*\bsrc="(\/assets\/[^"]+\.js)"/)?.[1]
      if (!entry) {
        throw new Error('vite-plugin-tauri-bridge: entry do renderer não encontrada no html gerado')
      }
      const bundle = readFileSync(resolve(outDir, entry.replace(/^\//, '')), 'utf8')
      if (!bundle.includes('mika_invoke')) {
        throw new Error(
          'vite-plugin-tauri-bridge: ponte ausente do bundle — window.electronAPI não existiria em runtime',
        )
      }

      // `check:packaged-assets` / `LICENSE-ASSETS.md`: as amostras Live2D em
      // `src/renderer/public/live2d` (`haru/`, `Mao/`) são material de terceiro sob a Live2D Free
      // Material License, que PROÍBE redistribuição num instalador assinado. O Vite copia todo
      // `public/**` para `out/renderer` — sem este passo, `tauri build` empacota o diretório junto
      // com o resto de `frontendDist` (não existe glob de exclusão equivalente ao antigo
      // `!out/renderer/live2d/**` do electron-builder no `tauri.conf.json`). O app já degrada
      // nomeado sem elas (ver LICENSE-ASSETS.md §"Como usar Live2D numa instalação" — o usuário
      // baixa e coloca em `<userData>/Mika/live2d/<nome>/`), então a UI não perde funcionalidade
      // nenhuma que dependa do bundle: só passa a exigir o passo manual documentado.
      const live2dOut = resolve(outDir, 'live2d')
      if (existsSync(live2dOut)) {
        rmSync(live2dOut, { recursive: true, force: true })
      }
    },
  }
}

function cspMetaPlugin() {
  return {
    name: 'vite-plugin-csp-meta',
    apply: 'build' as const,
    transformIndexHtml(html: string) {
      // `withTauriIpc`: com meta + header, o navegador aplica a INTERSEÇÃO — sem o `ipc:` aqui,
      // a meta anula o que a tauri.conf.json libera e TODO invoke é recusado pela CSP.
      const csp = withTauriIpc(process.env.MIKA_E2E ? DEV_CSP_META : PROD_CSP_META)
      return html.replace('</head>', `<meta http-equiv="Content-Security-Policy" content="${csp}"></head>`)
    },
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), tauriBridgePlugin(), cspMetaPlugin()],
  root: resolve(__dirname, 'src/renderer'),
  // Tauri lê a saída de fora do root do renderer; o preload também mora fora dele.
  server: {
    // 5273, não 5173: a porta padrão do Vite já estava ocupada por outro projeto da máquina,
    // e o binário debug do Tauri carrega `devUrl` — a janela subiu mostrando o app errado,
    // sem erro nenhum. `strictPort` faz a colisão falhar alto em vez de escorregar de porta.
    port: 5273,
    strictPort: true,
    fs: { allow: [resolve(__dirname, '..', '..')] },
  },
  resolve: {
    dedupe: ['zod'],
    alias: [
      // `react-dom` antes de `react`: alias em array casa por prefixo na ORDEM da lista, e
      // `react` primeiro sequestraria `react-dom` para node_modules/react + "-dom".
      { find: 'react-dom', replacement: resolve(__dirname, 'node_modules/react-dom') },
      { find: 'react', replacement: resolve(__dirname, 'node_modules/react') },
      { find: '@mika/sdk/contracts', replacement: resolve(__dirname, '../../packages/sdk/src/contracts.ts') },
      { find: '@mika/sdk-ui/settings', replacement: resolve(__dirname, '../../packages/sdk-ui/src/settings.ts') },
      { find: '@mika/sdk-ui', replacement: resolve(__dirname, '../../packages/sdk-ui/src/index.tsx') },
      { find: '@mika/sdk', replacement: resolve(__dirname, '../../packages/sdk/src') },
    ],
  },
  /**
   * Cinco componentes do renderer leem `process.env.NODE_ENV`. Sob Electron o preload rodava
   * em contexto Node e `process` existia; no webview do Tauri não existe. Substituir aqui, em
   * build, deixa essas leituras virarem literal e não depender de shim nenhum em runtime.
   */
  define: {
    'process.env.NODE_ENV': JSON.stringify(process.env.NODE_ENV ?? 'production'),

    /**
     * `process.platform` e `process.versions.*` são lidos por `preload/electron-shell-api.ts`.
     * Substituir em build, e não depender do shim injetado pelo Rust, porque o próprio Vite
     * injeta um `process` em dev e SOBRESCREVE o nosso — o app subia branco com
     * "Cannot read properties of undefined (reading 'node')".
     *
     * `node` e `electron` viram `undefined`, nunca string vazia: o
     * `live2dcubismcore.min.js` é emscripten e testa `typeof process.versions.node == "string"`
     * para decidir se está no Node. String vazia passa nesse teste e o faz chamar
     * `require('fs')`, matando o avatar. (Esse arquivo fica em public/ e é carregado por tag
     * de script, então não é afetado por estes defines — só pelo global em runtime.)
     */
    'process.platform': JSON.stringify(
      process.platform === 'win32' ? 'win32' : process.platform === 'darwin' ? 'darwin' : 'linux',
    ),
    'process.versions.node': 'undefined',
    'process.versions.chrome': '""',
    'process.versions.electron': 'undefined',
  },
  optimizeDeps: {
    include: ['zod'],
  },
  build: {
    outDir: resolve(__dirname, 'out/renderer'),
    emptyOutDir: true,
    cssCodeSplit: true,
    reportCompressedSize: false,
    chunkSizeWarningLimit: 600,
    rolldownOptions: {
      input: { index: writeTauriHtml() },
      output: {
        manualChunks: (id: string) => {
          if (id.includes('node_modules/three')) return 'three-vendor'
          if (id.includes('node_modules/katex')) return 'katex-vendor'
          if (id.includes('node_modules/@xmldom')) return 'xmldom-vendor'
          if (id.includes('node_modules/zustand')) return 'zustand-vendor'
          if (/node_modules\/(react|react-dom|scheduler)\//.test(id)) return 'react-vendor'
        },
      },
    },
  },
})
