/**
 * check:no-hardcoded-speech — pega string literal de fala passada aos funis de saída de voz
 * (`speakThroughTTS`, `sendProactive`, ...). Toda fala da Mika tem de ser gerada por LLM; guarda o
 * funil real, não a chamada interna, que já deixou passar cobertura falsa.
 */
import { isSource, isTest, lineOf, read, report, sourceRoots, type Violation, walk } from './lib.mts'

const FUNNELS = ['speakThroughTTS', 'sendProactive', 'speakNow', 'say']

const files = [
  ...walk('apps/desktop/src/main', (rel) => isSource(rel) && !isTest(rel)),
  ...walk('apps/desktop/src/features', (rel) => isSource(rel) && !isTest(rel)),
  ...sourceRoots()
    .filter((root) => root.startsWith('packages/'))
    .flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel))),
]

const violations: Violation[] = []

for (const file of files) {
  const source = read(file)

  for (const funnel of FUNNELS) {
    // Primeiro argumento literal: `funil('texto'` ou `funil("texto"` ou template sem `${`.
    const pattern = new RegExp(`\\b${funnel}\\s*\\(\\s*(?:'([^']{4,})'|"([^"]{4,})"|\`([^\`$]{4,})\`)`, 'g')

    for (const match of source.matchAll(pattern)) {
      const literal = match[1] ?? match[2] ?? match[3]
      violations.push({
        file,
        line: lineOf(source, match.index ?? 0),
        message: `${funnel}() recebe fala literal ("${literal.slice(0, 40)}…") — a fala da Mika é gerada por LLM`,
      })
    }
  }
}

report('check:no-hardcoded-speech', violations, files.length)
