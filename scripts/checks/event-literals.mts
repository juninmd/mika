/**
 * check:event-literals — pega evento do catálogo endereçado por string crua em vez da constante.
 * `check:events` casa emissor/ouvinte pelo VALOR, então typo idêntico dos dois lados fica invisível
 * lá; este gate cobre essa cegueira.
 */
import { isSource, isTest, lineOf, read, report, sourceRoots, type Violation, walk } from './lib.mts'

const CATALOG = 'packages/sdk/src/svc/events.ts'
const EXEMPT = new Set([
  CATALOG,
  'apps/desktop/src/shared/mika-event-channels.ts',
  'packages/sdk/src/contracts/desktop-ipc.ts',
])

const values = new Set<string>()
for (const match of read(CATALOG).matchAll(/^\s+[A-Z][A-Z0-9_]*:\s*'([^']+)'/gm)) {
  values.add(match[1])
}

const files = [...sourceRoots().flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel)))].filter(
  (rel) => !EXEMPT.has(rel),
)

const ADDRESSED =
  /(?:\.emit|\bemit|\bemitEvent|\bpublish|\.on|\.once|\.subscribe|\.off|\bregisterHandler)\s*\(\s*'([^']+)'/g

const violations: Violation[] = []

for (const file of files) {
  const source = read(file)
  for (const match of source.matchAll(ADDRESSED)) {
    if (!values.has(match[1])) continue
    violations.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message: `evento '${match[1]}' endereçado por string crua — use a constante de Events, senão um typo dos dois lados fica invisível`,
    })
  }
}

report('check:event-literals', violations, files.length)
