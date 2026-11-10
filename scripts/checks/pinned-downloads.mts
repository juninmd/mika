/**
 * check:pinned-downloads — a download URL in source never points at a mutable ref.
 * Whoever controls `master`/`main` would otherwise swap the bytes every new install executes.
 *
 * Os downloads de modelo vivem em Rust (`https://huggingface.co/{repo}/resolve/{REVISION}/…`), onde a
 * URL é montada por `format!` e o ref mora numa constante `*REVISION*` — um `"main"` ali não aparece
 * como literal de URL em lugar nenhum. Por isso o lado Rust confere as constantes, não só as URLs.
 */
import { isSource, isTest, lineOf, read, report, sourceRoots, type Violation, walk } from './lib.mts'

const MUTABLE_REF =
  /https:\/\/[^\s'"`]*\/(?:(?:resolve|raw)\/main|master|HEAD|refs\/heads)\/|raw\.githubusercontent\.com\/[^\s'"`/]+\/[^\s'"`/]+\/main\//g

const files = sourceRoots().flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel)))
const violations: Violation[] = []

for (const file of files) {
  const source = read(file)
  for (const match of source.matchAll(MUTABLE_REF)) {
    violations.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message: `download de ref móvel (${match[0]}) — fixe um SHA de commit ou uma tag`,
    })
  }
}

const RUST_ROOT = 'apps/desktop/src-tauri/src'
const REVISION_CONST = /\bconst\s+[A-Z0-9_]*REVISION\s*:\s*&(?:'static\s+)?str\s*=\s*"([^"]*)"/g
const COMMIT_SHA = /^[0-9a-f]{40}$/
const isRustTest = (rel: string): boolean => /(^|\/)tests?\.rs$|_tests?\.rs$/.test(rel)

/** Fonte de produção: sem linha de comentário (`//`, `///`) e sem o `mod tests` do fim do arquivo. */
function rustProduction(file: string): string {
  const source = read(file)
  const cut = source.indexOf('#[cfg(test)]')
  return (cut === -1 ? source : source.slice(0, cut))
    .split('\n')
    .map((line) => (line.trimStart().startsWith('//') ? '' : line))
    .join('\n')
}

const rustFiles = walk(RUST_ROOT, (rel) => rel.endsWith('.rs') && !isRustTest(rel))
let revisions = 0

for (const file of rustFiles) {
  const source = rustProduction(file)
  for (const match of source.matchAll(MUTABLE_REF)) {
    violations.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message: `download de ref móvel (${match[0]}) — fixe um SHA de commit ou uma tag`,
    })
  }
  for (const match of source.matchAll(REVISION_CONST)) {
    revisions++
    if (COMMIT_SHA.test(match[1] as string)) continue
    violations.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message: `revisão de download "${match[1]}" não é um SHA de commit de 40 hex — branch e tag mudam por baixo de quem instala`,
    })
  }
}

// Piso de alcance: um regex que pare de casar as constantes não pode sair verde em silêncio.
if (revisions < 5) {
  violations.push({
    file: 'scripts/checks/pinned-downloads.mts',
    line: 1,
    message: `só ${revisions} constante(s) *REVISION* reconhecida(s) em ${RUST_ROOT} — o regex parou de casar?`,
  })
}

report('check:pinned-downloads', violations, files.length + rustFiles.length)
