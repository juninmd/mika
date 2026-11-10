/**
 * check:dormant-modules — pega módulo escrito, testado e verde que nenhum caminho de produção chama
 * (só teste importa). "Export não usado" não pega isso porque o teste conta como consumidor.
 */
import { dirname, posix } from 'node:path'
import { exists, isSource, isTest, read, report, sourceRoots, type Violation, walk } from './lib.mts'

const ROOTS = sourceRoots()

// Dormência aceita, com a razão — decisão explícita no diff, não acidente silencioso.
const ACCEPTED: ReadonlyMap<string, string> = new Map([
  [
    'apps/desktop/src/shared/csp-policy.ts',
    'importado por vite.tauri.config.ts (fora de sourceRoots) para a <meta> CSP do build — não por caminho de produção dentro de src/',
  ],
  /*
   * O chokepoint do renderer é ligado por SUBSTITUIÇÃO no bundler, não por import literal: quem
   * importa `shared/electron-runtime` recebe este arquivo no lugar. Como nenhum
   * `from '...tauri-renderer-runtime'` existe no fonte, este gate só o enxerga quando um teste o
   * importa — e aí o acusa de dormente, justamente por ter ganhado teste.
   *
   * A ligação é verificável no `resolveId`/alias de `vite.tauri.config.ts`.
   *
   * Havia um segundo, `tauri-main-runtime.ts`, ligado pelo `onResolve` de `build-sidecar.mts`:
   * ele emulava a API do Electron para o backend Node rodar sob Tauri. Os dois saíram em
   * 2026-09-22 junto com o sidecar — sem processo Node, a emulação não tinha o que servir.
   */
  [
    'apps/desktop/src/shared/tauri-renderer-runtime.ts',
    'substituído por alias em vite.tauri.config.ts — quem importa shared/electron-runtime recebe este arquivo',
  ],
])

const all = ROOTS.flatMap((root) => walk(root, isSource))
const sources = all.filter((rel) => !isTest(rel))
const tests = all.filter(isTest)

// Convenção de exportação dos pacotes (`"./*": "./src/*.ts"`); sem isto, import via subpath
// público (`@mika/sdk/utils/...`) virava falso positivo de "só teste importa".
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

for (const file of sources) {
  if (importedByProduction.has(file)) continue
  if (!importedByTest.has(file)) continue
  if (ACCEPTED.has(file)) continue

  violations.push({
    file,
    line: 1,
    message:
      'o único importador deste módulo é um teste — nenhum caminho de produção o chama. Ligue-o, apague-o, ou declare a dormência em scripts/checks/dormant-modules.mts',
  })
}

report('check:dormant-modules', violations, sources.length + tests.length)
