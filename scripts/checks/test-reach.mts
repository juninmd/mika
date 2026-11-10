/**
 * check:test-reach — catraca de regressão: quantos arquivos os testes de unidade alcançam (via
 * imports a partir dos testes) não pode cair sem ninguém notar. Não é meta de cobertura absoluta —
 * boa parte do não-alcançado é renderer, provado por E2E, não Vitest.
 */
import { dirname, posix } from 'node:path'
import { exists, isSource, isTest, read, report, sourceRoots, type Violation, walk } from './lib.mts'

/**
 * Medido em 2026-09-10: 450 de 824 fontes alcançadas.
 * Remedido em 2026-09-22 após Stage C (deleção do backend Electron/Node — `src/main`, `src/preload`
 * antigo e o não-renderer de `src/features/*`): 350 fontes. A base encolheu porque as fontes
 * alcançáveis encolheram junto — não é regressão de cobertura, é o denominador mudando.
 * Remedido de novo no mesmo dia, depois do audit de `check:events`: `packages/scheduler` era
 * backend Node morto (zero import fora do próprio pacote, `@mika/scheduler` nem listado nas deps
 * de `apps/desktop`, porte Rust completo em `modules/features/scheduler`) e foi deletado — seus
 * próprios fontes e testes saíram do denominador junto. 343 fontes.
 * Remedido em 2026-09-23: o legado Node do SDK (container, plugins, store/libsql, tools, http, host),
 * `packages/llm`, `packages/mood` e os 5 `character-schema/*.schema.ts` saíram com seus testes
 * (~140 fontes). Nenhum teste apagado alcançava fonte sobrevivente que não seja alcançada por outro.
 * 224 fontes.
 * Remedido em 2026-10-06: o piso ficou em 224 enquanto o alcance real subia (renderer, i18n e
 * features novas com testes) — catraca parada em 224 deixava ~77 fontes caírem sem aviso. Medido:
 * 301 fontes (depois da poda de 5 testes fuzzy tautológicos de `packages/sdk`, que só liam constantes
 * já tipadas e não alcançavam fonte que outro teste não alcance). Catraca só sobe: nunca abaixe.
 */
const PISO = 301

const all = sourceRoots().flatMap((root) => walk(root, isSource))
const sources = all.filter((file) => !isTest(file))

function resolveImport(from: string, spec: string): string | null {
  if (!spec.startsWith('.')) return null
  const base = posix.normalize(posix.join(dirname(from), spec))
  for (const candidate of [`${base}.ts`, `${base}.tsx`, `${base}/index.ts`, `${base}/index.tsx`, base]) {
    if (exists(candidate) && isSource(candidate)) return candidate
  }
  return null
}

const imports = new Map<string, string[]>()
for (const file of all) {
  const out: string[] = []
  for (const match of read(file).matchAll(/(?:from|import)\s*\(?\s*['"](\.[^'"]+)['"]/g)) {
    const resolved = resolveImport(file, match[1] as string)
    if (resolved !== null) out.push(resolved)
  }
  imports.set(file, out)
}

const reached = new Set<string>()
const queue = all.filter(isTest)
while (queue.length > 0) {
  const file = queue.pop() as string
  for (const dep of imports.get(file) ?? []) {
    if (!reached.has(dep)) {
      reached.add(dep)
      queue.push(dep)
    }
  }
}

const alcance = sources.filter((file) => reached.has(file)).length
const violations: Violation[] = []

if (alcance < PISO) {
  violations.push({
    file: 'scripts/checks/test-reach.mts',
    line: 1,
    message: `a suíte de unidade alcança ${alcance} fontes, abaixo do piso de ${PISO}. Algum teste deixou de exercitar código que exercitava — cubra de novo, ou baixe o piso no mesmo commit dizendo por quê`,
  })
}

report('check:test-reach', violations, alcance)
