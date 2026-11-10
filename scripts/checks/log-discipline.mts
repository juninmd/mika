/**
 * check:log-discipline — pega `console.*` em `features/`/`packages/` fora do sink de log (sem
 * sink de arquivo, sem nível, sem redação). Isento: build, testes e o próprio sink de log.
 */
import { isSource, isTest, lineOf, read, report, sourceRoots, type Violation, walk } from './lib.mts'

// `apps/desktop/src/main` saiu do gate: apagado no Stage C da migração Tauri (2026-09-22), então
// `walk()` sobre ele sempre devolvia `[]` — raiz morta, não piso de alcance real (ver
// `specs/10-observability.md`, "Formato").
const files = [
  ...walk('apps/desktop/src/features', (rel) => isSource(rel) && !isTest(rel)),
  ...sourceRoots()
    .filter((root) => root.startsWith('packages/') && root !== 'packages/sdk-ui/src')
    .flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel))),
].filter((rel) => !rel.includes('/renderer/'))

const violations: Violation[] = []

// `console.log(` dentro de string literal é texto, não chamada (ex: mensagem de erro com um
// comando de exemplo); conta aspas abertas antes do match para distinguir sem parser.
function insideStringLiteral(source: string, index: number): boolean {
  const before = source.slice(source.lastIndexOf('\n', index) + 1, index)
  return ["'", '"', '`'].some((quote) => {
    let open = false
    for (let i = 0; i < before.length; i++) {
      if (before[i] === '\\') i++
      else if (before[i] === quote) open = !open
    }
    return open
  })
}

for (const file of files) {
  const source = read(file)
  for (const match of source.matchAll(/\bconsole\.(log|info|warn|error|debug|trace)\s*\(/g)) {
    if (insideStringLiteral(source, match.index ?? 0)) continue
    violations.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message: `console.${match[1]} em código compartilhado — não chega ao sink de arquivo nem é redigido`,
    })
  }
}

report('check:log-discipline', violations, files.length)
