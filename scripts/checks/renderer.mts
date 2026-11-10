/**
 * check:renderer — isolamento de processo.
 *
 * `specs/01-architecture.md`: o renderer MUST NOT importar lib Node, e MUST importar apenas de
 * `@mika/sdk-ui` e `@mika/sdk/contracts` — o entrypoint `@mika/sdk` arrasta Node para o browser.
 *
 * Isento: `features/<x>/renderer/` e `renderer/settings/<x>/` também rodam no renderer, então
 * entram na varredura; o que NÃO entra é o preload, que é ponte por definição.
 */
import { builtinModules } from 'node:module'
import { isSource, isTest, lineOf, read, report, type Violation, walk } from './lib.mts'

// Todo módulo embutido do Node, com ou sem o prefixo `node:`, mais `electron`. A lista escrita à mão
// deixava passar `node:fs/promises`, `node:url`, `node:util`, `os`, `crypto`, `http`…
const NODE_BUILTINS = new Set([...builtinModules, 'electron'])
const isNodeBuiltin = (spec: string): boolean => spec.startsWith('node:') || NODE_BUILTINS.has(spec.split('/')[0])

const ROOTS = ['apps/desktop/src/renderer/src', 'apps/desktop/src/renderer/settings', 'packages/sdk-ui/src']
const FEATURE_UI = /^apps\/desktop\/src\/features\/[^/]+\/renderer\//

const files = [
  ...ROOTS.flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel))),
  ...walk('apps/desktop/src/features', (rel) => isSource(rel) && !isTest(rel) && FEATURE_UI.test(rel)),
]

const violations: Violation[] = []

for (const file of files) {
  const source = read(file)

  // `import type` é apagado na compilação: não chega ao bundle, logo não viola isolamento.
  // Preserva as quebras de linha para o número de linha do relatório continuar verdadeiro.
  const scanned = source.replace(/import\s+type\s+[^;]*?from\s*['"][^'"]+['"]/g, (hit) => hit.replace(/[^\n]/g, ' '))

  for (const match of scanned.matchAll(/(?:from|import)\s*\(?\s*['"]([^'"]+)['"]/g)) {
    const spec = match[1]
    const index = match.index ?? 0

    if (isNodeBuiltin(spec)) {
      violations.push({
        file,
        line: lineOf(scanned, index),
        message: `renderer importa "${spec}" — código de processo main no browser`,
      })
      continue
    }

    if (spec === '@mika/sdk') {
      violations.push({
        file,
        line: lineOf(scanned, index),
        message: 'renderer importa o entrypoint "@mika/sdk" — use "@mika/sdk/contracts" ou "@mika/sdk-ui"',
      })
    }
  }

  // A ponte para o main é `getElectronApi()`, e o valor dela é ser um ponto único: um lugar que
  // falha com erro nomeado quando o preload não subiu, e um lugar para envolver. Uma exceção já
  // basta para deixar de ser ponto único — e havia exatamente uma.
  if (!file.endsWith('lib/shell-api.ts')) {
    for (const match of scanned.matchAll(/\bwindow\.electronAPI\b/g)) {
      violations.push({
        file,
        line: lineOf(scanned, match.index ?? 0),
        message: 'acessa window.electronAPI direto — use getElectronApi() de lib/shell-api',
      })
    }
  }
}

report('check:renderer', violations, files.length)
