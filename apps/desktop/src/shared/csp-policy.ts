// Esquema custom `mika-live2d` (`src-tauri/src/live2d_protocol.rs`): serve os modelos Live2D que o
// usuário instala manualmente em `<userData>/live2d/<nome>/` (LICENSE-ASSETS.md) — as amostras não
// vão no instalador. `http://mika-live2d.localhost` é a forma Windows/Android; `mika-live2d:` (só
// o esquema, sintaxe de CSP scheme-source) cobre a forma `mika-live2d://localhost` de macOS/Linux.
const LIVE2D_SCHEME_SOURCES = 'http://mika-live2d.localhost mika-live2d:'

export const PROD_CSP_DIRECTIVES = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  `img-src 'self' data: blob: ${LIVE2D_SCHEME_SOURCES}`,
  "media-src 'self' data: blob: https://storage.googleapis.com",
  `connect-src 'self' ${LIVE2D_SCHEME_SOURCES}`,
  "font-src 'self'",
  "worker-src 'self' blob:",
  "frame-ancestors 'none'",
  "base-uri 'self'",
  "form-action 'none'",
] as const

export const DEV_CSP_DIRECTIVES = [
  "default-src 'self' 'unsafe-inline' 'unsafe-eval' http://localhost:* http://127.0.0.1:* ws://localhost:* ws://127.0.0.1:*",
  "script-src 'self' 'unsafe-inline' 'unsafe-eval' http://localhost:* http://127.0.0.1:*",
  "style-src 'self' 'unsafe-inline'",
  "font-src 'self'",
  `connect-src 'self' ws://localhost:* http://localhost:* ws://127.0.0.1:* http://127.0.0.1:* ${LIVE2D_SCHEME_SOURCES}`,
  `img-src 'self' data: blob: ${LIVE2D_SCHEME_SOURCES}`,
  "media-src 'self' data: blob: https://storage.googleapis.com",
  'worker-src blob:',
  "frame-ancestors 'none'",
] as const

export const PROD_CSP: string = PROD_CSP_DIRECTIVES.join('; ')
export const DEV_CSP: string = DEV_CSP_DIRECTIVES.join('; ')

const META_IGNORED_DIRECTIVES = ['frame-ancestors', 'report-uri', 'sandbox'] as const

function toMetaPolicy(directives: readonly string[]): string {
  return directives.filter((d) => !META_IGNORED_DIRECTIVES.some((ignored) => d.startsWith(ignored))).join('; ')
}

export const PROD_CSP_META: string = toMetaPolicy(PROD_CSP_DIRECTIVES)
export const DEV_CSP_META: string = toMetaPolicy(DEV_CSP_DIRECTIVES)

/**
 * Sob Tauri o IPC é um `fetch` para `http://ipc.localhost` — `connect-src`, portanto, e não um
 * canal fora do alcance da CSP como o `ipcRenderer` do Electron.
 *
 * Quando a página tem `<meta>` CSP e o servidor manda outra, o navegador aplica a INTERSEÇÃO. A
 * meta gerada daqui valia `connect-src 'self'` e anulava o `ipc:` que a `tauri.conf.json` já
 * liberava: todo `invoke` era recusado pela CSP, o Tauri caía no fallback por `postMessage`, e o
 * app abria com tema, i18n, config, histórico e health todos falhando. Uma CSP restritiva demais
 * não é "mais segura" — aqui ela desligava o produto inteiro.
 *
 * Esta lista tem de continuar batendo com `app.security.csp` da `tauri.conf.json`; é o teste
 * `csp-policy.test.ts` que lê as duas e cobra isso.
 */
export const TAURI_IPC_SOURCES = 'ipc: http://ipc.localhost'

export function withTauriIpc(policy: string): string {
  return policy.replace(
    /connect-src ([^;]*)/,
    (_match, sources: string) => `connect-src ${sources} ${TAURI_IPC_SOURCES}`,
  )
}
