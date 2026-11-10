/**
 * check:dead-deps — pega dependência declarada que nenhuma linha do repo importa (gate de superfície:
 * dependência morta é CVE que ninguém lê, não peso de bundle). Pergunta feita sobre o repo INTEIRO,
 * não por workspace, para não acusar duplicação deliberada entre pacotes.
 */
import { isSource, read, report, sourceRoots, type Violation, walk, workspaceManifests } from './lib.mts'

const MANIFESTS = workspaceManifests()

const SOURCE_ROOTS = [...sourceRoots(), 'apps/desktop/e2e', 'scripts']

/** Consumidas por configuração ou runtime, nunca por `import`. */
const IMPLICIT = new Set([
  '@fontsource-variable/inter',
  '@fontsource/ibm-plex-mono',
  '@fontsource/space-grotesk',
  '@mika/sdk',
  '@mika/sdk-ui',
  'rimraf',
  'tailwindcss',
  'typescript',
  'vite',
])

const sources = SOURCE_ROOTS.flatMap((root) => walk(root, isSource))
  .map((rel) => read(rel))
  .join('\n')
  // Linha só de comentário (`//`, `///`, `//!`) não é uso: `// ver tokio::spawn` mantinha viva uma crate morta.
  .split('\n')
  .filter((line) => !line.trimStart().startsWith('//'))
  .join('\n')

const violations: Violation[] = []
const seen = new Set<string>()

for (const manifest of MANIFESTS) {
  const parsed = JSON.parse(read(manifest)) as { dependencies?: Record<string, string> }

  for (const dep of Object.keys(parsed.dependencies ?? {})) {
    if (IMPLICIT.has(dep) || seen.has(dep)) continue
    seen.add(dep)

    const escaped = dep.replace(/[/\\^$*+?.()|[\]{}]/g, '\\$&')
    if (new RegExp(`['"]${escaped}(/[^'"]*)?['"]`).test(sources)) continue

    violations.push({
      file: manifest,
      line: 1,
      message: `"${dep}" é dependência declarada que nenhuma linha do repositório importa`,
    })
  }
}

/**
 * Rust: crate declarada em `apps/desktop/src-tauri/Cargo.toml` que nenhum `.rs` (nem `build.rs`)
 * nomeia. Procura o nome da crate (traço vira sublinhado) como caminho (`nome::`, inclusive em atributo/macro),
 * `use nome` ou `extern crate nome` — o mesmo critério do `import` do TS.
 * Pertence aqui, e não a um teste Rust, porque a pergunta é sobre o MANIFESTO inteiro, não sobre
 * um objeto em runtime: `dashmap` e `rubato` ficaram declaradas, sem uma linha de uso, até
 * esta checagem existir.
 */
const CARGO_MANIFEST = 'apps/desktop/src-tauri/Cargo.toml'
const RUST_SOURCE_ROOTS = ['apps/desktop/src-tauri/src']

/** Consumidas sem nome em `.rs`: o nome vem de feature/build, nunca de um caminho de código. */
const RUST_IMPLICIT = new Set<string>([
  // `#[derive(Trace, Finalize)]` (`code_runner/logger.rs`) expande para um caminho absoluto
  // `boa_gc::...`; o nome nunca aparece escrito no fonte (ver o comentário em Cargo.toml).
  'boa_gc',
])

const rustSources = [
  ...RUST_SOURCE_ROOTS.flatMap((root) => walk(root, (rel) => rel.endsWith('.rs'))),
  'apps/desktop/src-tauri/build.rs',
]
  .map((rel) => read(rel))
  .join('\n')

const DEP_SECTION = /^\[((?:target\.[^\]]+\.)?(?:dev-|build-)?dependencies)\]\s*$/
let inDependencies = false

for (const [index, line] of read(CARGO_MANIFEST).split('\n').entries()) {
  const section = line.trim().match(/^\[(.+)\]$/)
  if (section) inDependencies = DEP_SECTION.test(line.trim())
  const declared = inDependencies ? line.match(/^([A-Za-z0-9_-]+)\s*=/) : null
  if (!declared) continue

  const crate = declared[1]
  if (RUST_IMPLICIT.has(crate) || seen.has(`cargo:${crate}`)) continue
  seen.add(`cargo:${crate}`)

  const ident = crate.replace(/-/g, '_')
  if (new RegExp(`\\b${ident}::|\\buse\\s+\\{?\\s*${ident}\\b|extern\\s+crate\\s+${ident}\\b`).test(rustSources))
    continue

  violations.push({
    file: CARGO_MANIFEST,
    line: index + 1,
    message: `crate "${crate}" é dependência declarada que nenhum .rs do shell referencia`,
  })
}

report('check:dead-deps', violations, seen.size)
