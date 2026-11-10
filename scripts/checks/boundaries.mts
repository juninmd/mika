/**
 * check:boundaries — pega import relativo que alcança o interior de outro módulo em vez da API
 * pública (`index.ts`/token de DI), e núcleo importando feature diretamente.
 */
import { dirname, posix } from 'node:path'
import { isSource, isTest, lineOf, read, report, type Violation, walk } from './lib.mts'

const MODULE_OF = [/^(apps\/desktop\/src\/main\/services\/[^/]+)\//, /^(apps\/desktop\/src\/features\/[^/]+)\//]

const moduleOf = (rel: string): string | null => {
  for (const pattern of MODULE_OF) {
    const match = pattern.exec(rel)
    if (match) return match[1]
  }
  return null
}

const isCore = (mod: string): boolean => mod.startsWith('apps/desktop/src/main/services/')
const isFeature = (mod: string): boolean => mod.startsWith('apps/desktop/src/features/')

// Raiz de composição isenta: `bootstrap/` precisa ver implementações concretas para montar o grafo.
const COMPOSITION_ROOT = 'apps/desktop/src/main/services/bootstrap/'

const files = [
  ...walk('apps/desktop/src/main/services', (rel) => isSource(rel) && !isTest(rel)),
  ...walk('apps/desktop/src/features', (rel) => isSource(rel) && !isTest(rel)),
].filter((rel) => !rel.startsWith(COMPOSITION_ROOT))

const violations: Violation[] = []

for (const file of files) {
  const home = moduleOf(file)
  if (home === null) continue

  const source = read(file)

  for (const match of source.matchAll(/from\s*['"](\.[^'"]+)['"]/g)) {
    const spec = match[1]
    const resolved = posix.normalize(posix.join(dirname(file), spec))
    const target = moduleOf(resolved)

    if (target === null || target === home) continue

    const line = lineOf(source, match.index ?? 0)
    const publicApi = `${target}/index`
    const reachesInside = resolved !== publicApi && !resolved.startsWith(`${publicApi}.`)

    if (isCore(home) && isFeature(target)) {
      violations.push({
        file,
        line,
        message: `núcleo importa a feature "${target.split('/').pop()}" — resolva por token de DI tolerando ausência`,
      })
      continue
    }

    if (reachesInside) {
      violations.push({
        file,
        line,
        message: `alcança o interior de outro módulo (${resolved}) — importe a API pública ${publicApi}`,
      })
    }
  }
}

report('check:boundaries', violations, files.length)
