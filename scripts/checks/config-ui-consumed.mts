/**
 * check:config-ui-consumed — "UI → runtime", a direção que faltava. `check:config-read-paths` pega
 * leitura sem campo no schema; `check:config-schema-parity` pega campo sem controle na UI. Nenhum
 * pegava o controle que aparece na tela e NINGUÉM lê: a varredura de 2026-09-27 achou mais de 40
 * (topP/thinking que nunca chegavam a Google/OpenRouter, roteador de modelos nunca portado,
 * tradução DeepLX, Kokoro cortado, TTLs de memória, toggles mestres de feature que não desligavam
 * nada). Controle morto é pior que controle ausente: o usuário mexe, nada muda, e ele passa a
 * desconfiar dos controles que funcionam.
 *
 * Controle = `configPath` literal na UI de settings. Leitor = o mesmo caminho como literal no Rust de
 * produção (fora de `config/schemas/`, de testes e das linhas que só DECLARAM o campo para a tela)
 * ou no TypeScript do renderer fora das telas de settings.
 */
import { lineOf, read, report, type Violation, walk } from './lib.mts'

const UI_ROOTS = [
  'apps/desktop/src/renderer/settings',
  'apps/desktop/src/renderer/src/components/settings',
  'apps/desktop/src/features',
]
const isUiSource = (rel: string): boolean =>
  /\.(ts|tsx)$/.test(rel) &&
  !/__tests__|\.test\.|\.spec\./.test(rel) &&
  (rel.includes('/renderer/') || rel.includes('/settings/'))
const isSettingsUi = (rel: string): boolean => UI_ROOTS.some((root) => rel.startsWith(root))

const CONTROL = /configPath(?:=\{?|:\s*)["']([a-zA-Z][\w-]*(?:\.[\w-]+)+)["']/g
const LITERAL = /["'`]([a-zA-Z][\w-]*(?:\.[\w-]+)+)["'`]/g
/** Linha que só DECLARA o campo para a tela (lista de campos, visibleWhen) não é leitura. */
const DECLARATION = /configPath|enabledConfigPath|visibleWhen/

/** Lido por caminho montado em runtime, não por literal — namespace + onde, nunca por campo. */
const DYNAMIC_READERS: ReadonlyMap<string, string> = new Map([
  ['user.', 'brain/persona/user_context.rs lê o namespace `user` inteiro e injeta no prompt'],
  [
    'gamer-companion.momentCooldowns.',
    'gamer/game_moment_commentary.rs lê o mapa `momentCooldowns` e indexa pelo tipo do momento',
  ],
])

/** Comentário que CITA o caminho não o lê — foi assim que `privacy.historyRetention` passou. */
const COMMENT = /^\s*(\/\/|\/?\*)/

function literalsIn(source: string, into: Set<string>): void {
  for (const line of source.split('\n')) {
    if (DECLARATION.test(line) || COMMENT.test(line)) continue
    for (const match of line.matchAll(LITERAL)) into.add(match[1] as string)
  }
}

const RUST_ROOT = 'apps/desktop/src-tauri/src'
const RUST_CONTROL = /"configPath":\s*"([a-zA-Z][\w-]*(?:\.[\w-]+)+)"/g

/** Só o bloco `mod x {` de teste sai; `#[cfg(test)] mod x;` no topo cortava a produção inteira. */
function productionRust(source: string): string {
  const cut = source.search(/#\[cfg\(test\)\]\s*mod\s+\w+\s*\{/)
  return cut === -1 ? source : source.slice(0, cut)
}

const consumed = new Set<string>()
for (const file of walk(RUST_ROOT, (rel) => rel.endsWith('.rs'))) {
  if (file.includes('/config/schemas/') || /_tests?\.rs$|test_support\.rs$/.test(file)) continue
  literalsIn(productionRust(read(file)), consumed)
}
for (const file of walk('apps/desktop/src', (rel) => /\.(ts|tsx)$/.test(rel))) {
  if (isSettingsUi(file) || /__tests__|\.test\.|\.spec\./.test(file)) continue
  literalsIn(read(file), consumed)
}

const violations: Violation[] = []
let controls = 0
function checkControls(file: string, source: string, pattern: RegExp): void {
  for (const match of source.matchAll(pattern)) {
    controls++
    const path = match[1] as string
    if (consumed.has(path) || [...DYNAMIC_READERS.keys()].some((prefix) => path.startsWith(prefix))) continue
    violations.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message: `controle "${path}" aparece na tela mas nenhum código de runtime lê esse caminho — ligue-o ou remova-o`,
    })
  }
}
for (const root of UI_ROOTS) {
  for (const file of walk(root, isUiSource)) checkControls(file, read(file), CONTROL)
}
// Campo declarado ao registry em Rust (`feature.rs`/`feature_fields.rs`) também vira controle na
// tela — foi por aí que `llm.maxCostUsdPerTurn` apareceu sem leitor.
for (const file of walk(
  RUST_ROOT,
  (rel) => /(^|\/)feature[\w/]*\.rs$/.test(rel) || /\/feature_fields\.rs$/.test(rel),
)) {
  checkControls(file, productionRust(read(file)), RUST_CONTROL)
}

// Piso de alcance: um regex que pare de casar não pode sair verde em silêncio.
if (controls < 100) {
  violations.push({
    file: 'scripts/checks/config-ui-consumed.mts',
    line: 1,
    message: `só ${controls} controles encontrados — o regex de configPath parou de casar?`,
  })
}

report('check:config-ui-consumed', violations, controls)
