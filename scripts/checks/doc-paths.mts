/**
 * check:doc-paths — pega caminho de arquivo citado em doc que não existe (defeito, não imprecisão
 * de estilo). Saídas legítimas: riscar (`~~caminho~~`) ou doc ser registro histórico isento.
 * Também confere a contagem e o inventário de gates citados no README e na spec 11.
 */
import { posix } from 'node:path'
import { exists, lineOf, read, report, type Violation, walk } from './lib.mts'

/** Registro histórico: livro-razão, auditoria, homologação, checklist de migração, plano datado. */
const HISTORICAL = [
  /(^|\/)CHECKPOINT\.md$/i,
  /(^|\/)HOMOLOGATION/i,
  /\/report\//,
  /^\.workflow\//,
  /(^|\/)CHANGELOG\.md$/i,
  /^THIRD-PARTY-NOTICES\.md$/, // texto de licença de terceiros gerado (`notices:gen`), não doc nosso
]

const files = walk('.', (rel) => rel.endsWith('.md') && !HISTORICAL.some((p) => p.test(rel)))

/**
 * Um caminho de verdade tem barra e extensão conhecida, ou é uma pasta do repo com barra final.
 * `:linha` (ou `:linha-linha`) colado ao caminho é citação de linha, não parte do nome — o regex o
 * aceita e a resolução o descarta; sem isto `` `modules/x.rs:12` `` escapava do gate por não casar.
 */
const CITED =
  /`([a-zA-Z0-9_@./-]+\/[a-zA-Z0-9_@./-]*\.(?:ts|tsx|mts|mjs|js|json|md|css|html|wasm|node|onnx|rs|toml|yml|yaml))(?::\d+(?:-\d+)?)?`/g

/**
 * Onde um caminho citado pode ser resolvido: a raiz do repo (forma canônica) e as pastas a partir das
 * quais os docs abreviam caminhos Rust (`modules/brain/turn.rs`, `brain/turn.rs`, `src/steam.rs`).
 * A pasta do próprio doc também vale. Frouxo de propósito: reescrever dezenas de docs para a forma
 * canônica custa mais do que o que este atalho deixa passar.
 */
const RESOLVE_FROM = [
  '',
  'apps/desktop/',
  'apps/desktop/src/',
  'apps/desktop/src-tauri/',
  'apps/desktop/src-tauri/src/',
  'apps/desktop/src-tauri/src/modules/',
  'apps/desktop/src-tauri/src/modules/features/',
]

const violations: Violation[] = []

for (const file of files) {
  const source = read(file)
  const struck = new Set<string>()
  for (const match of source.matchAll(/~~([^~]+)~~/g)) struck.add(match[1].replace(/`/g, ''))

  for (const match of source.matchAll(CITED)) {
    const cited = match[1]
    if (struck.has(cited)) continue
    // Caminho relativo a partir da raiz do repo é a forma canônica; um `./algo.ts` solto é exemplo.
    if (cited.startsWith('./') || cited.startsWith('../')) continue
    if (RESOLVE_FROM.some((base) => exists(`${base}${cited}`)) || exists(posix.join(posix.dirname(file), cited)))
      continue

    violations.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message: `cita "${cited}", que não existe — corrija ou risque com ~~til duplo~~`,
    })
  }
}

/*
 * Contagem e inventário de gates: o README dizia "26 gates" e a spec 11 "36" quando `pnpm gates` já
 * rodava 40 — número escrito à mão envelhece. A fonte é a cadeia `gates` do `package.json`; README e
 * spec têm de citar cada `check:*` dela e, quando dizem quantos são, dizer o número certo.
 */
const gateChain = (JSON.parse(read('package.json')) as { scripts: { gates: string } }).scripts.gates
const gateNames = [...new Set(gateChain.match(/check:[a-z0-9-]+/g) ?? [])]
for (const [file, count] of [
  ['README.md', /Os (\d+) gates/],
  ['specs/11-quality-gates.md', /(\d+) gates de invariante/],
] as const) {
  const source = read(file)
  const said = count.exec(source)
  if (said && Number(said[1]) !== gateNames.length) {
    violations.push({
      file,
      line: lineOf(source, said.index),
      message: `diz ${said[1]} gates, mas \`pnpm gates\` roda ${gateNames.length} \`check:*\``,
    })
  }
  for (const name of gateNames.filter((gate) => !source.includes(`\`${gate}\``))) {
    violations.push({ file, line: 1, message: `não documenta o gate \`${name}\`, que \`pnpm gates\` roda` })
  }
}

report('check:doc-paths', violations, files.length)
