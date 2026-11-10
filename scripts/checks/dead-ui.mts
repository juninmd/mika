/**
 * check:dead-ui — pega peça de UI React sem importador de PRODUÇÃO (compila e passa em review
 * "parecendo" ligada, mas nada monta). Consumidor só em teste não conta. `*.module.ts` e
 * entrypoints puros (main.tsx, pacote "main"/"exports") escapam por convenção, não import estático.
 */
import { dirname, posix } from 'node:path'
import { exists, isSource, isTest, read, report, type Violation, walk } from './lib.mts'

const ROOTS = ['apps/desktop/src/renderer/src', 'apps/desktop/src/features', 'packages/sdk-ui/src']

const FEATURE_UI = /^apps\/desktop\/src\/features\/[^/]+\/renderer\//
const SHELL_UI = /^apps\/desktop\/src\/renderer\/src\//
const SDK_UI = /^packages\/sdk-ui\/src\//

function inScope(rel: string): boolean {
  return SHELL_UI.test(rel) || FEATURE_UI.test(rel) || SDK_UI.test(rel)
}

const isModuleManifest = (rel: string): boolean => rel.endsWith('.module.ts')

/** Entrypoint carregado fora do grafo de `import` de TS — arquivo + motivo, nunca por linha. */
const ACCEPTED: ReadonlyMap<string, string> = new Map([
  ['apps/desktop/src/renderer/src/main.tsx', 'entrypoint carregado por index.html, não por import'],
  [
    'apps/desktop/src/renderer/src/tauri-bridge-entry.ts',
    'entrypoint do alvo Tauri, carregado por index.tauri.html (derivado em vite.tauri.config.ts)',
  ],
  ['packages/sdk-ui/src/index.tsx', 'export principal do pacote (package.json "main"/"exports")'],
])

const all = walk('.', isSource)
const targets = ROOTS.flatMap((root) =>
  walk(root, (rel) => isSource(rel) && !isTest(rel) && !isModuleManifest(rel)),
).filter(inScope)

const PACKAGE_IMPORT = /^@mika\/([a-z-]+)\/(.+)$/

function packagePath(spec: string): string | null {
  const match = PACKAGE_IMPORT.exec(spec)
  return match === null ? null : `packages/${match[1]}/src/${match[2]}`
}

function resolveImport(fromFile: string, spec: string): string | null {
  const base = spec.startsWith('.') ? posix.normalize(posix.join(dirname(fromFile), spec)) : packagePath(spec)
  if (base === null) return null

  for (const candidate of [`${base}.ts`, `${base}.tsx`, `${base}/index.ts`, `${base}/index.tsx`, base]) {
    if (exists(candidate) && isSource(candidate)) return candidate
  }
  return null
}

const importedByProduction = new Set<string>()
const importedByTest = new Set<string>()

for (const file of all) {
  const source = read(file)
  const target = isTest(file) ? importedByTest : importedByProduction

  for (const match of source.matchAll(/(?:from|import)\s*\(?\s*['"]((?:\.|@mika\/)[^'"]+)['"]/g)) {
    const resolved = resolveImport(file, match[1])
    if (resolved !== null) target.add(resolved)
  }
}

const violations: Violation[] = []

for (const file of targets) {
  if (importedByProduction.has(file)) continue
  if (ACCEPTED.has(file)) continue

  const reason = importedByTest.has(file)
    ? 'só um teste importa este arquivo de UI — nenhum caminho de produção o monta'
    : 'nenhum arquivo importa esta peça de UI — nem produção, nem teste'

  violations.push({
    file,
    line: 1,
    message: `${reason}. Ligue-a, apague-a, ou declare a exceção em scripts/checks/dead-ui.mts`,
  })
}

// Piso de alcance: um filtro de raiz errado que passe a varrer poucos arquivos não deve sair verde.
if (targets.length < 200) {
  violations.push({
    file: 'scripts/checks/dead-ui.mts',
    line: 1,
    message: `varreu só ${targets.length} arquivo(s) — piso de alcance quebrado, confira ROOTS/inScope antes de confiar no resultado`,
  })
}

report('check:dead-ui', violations, targets.length)
