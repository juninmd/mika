/**
 * Base compartilhada dos gates de `scripts/checks/`.
 *
 * Um gate é um script que faz UMA pergunta sobre a árvore e sai 1 se a resposta for errada.
 * `specs/11-quality-gates.md` é a fonte do que cada um protege e de por que ele existe.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative, resolve, sep } from 'node:path'

export const REPO_ROOT = resolve(import.meta.dirname, '..', '..')

const SKIP_DIRS = new Set([
  'node_modules',
  '.git',
  'dist',
  'out',
  // Saída de build do Cargo: `bundle.resources` copia EULA/PRIVACY/prompts para `target/<perfil>/`, e a
  // cópia velha de um .md seria varrida como se fosse fonte.
  'target',
  'coverage',
  '.turbo',
  'test-results',
  'playwright-report',
  '.workflow',
  '.hermes',
  '.claude',
])

/** Um diretório ilegível é o mesmo que um diretório vazio para a pergunta de qualquer gate. */
function entriesOf(dir: string): ReturnType<typeof readdirSync> {
  try {
    return readdirSync(dir, { withFileTypes: true })
  } catch {
    return []
  }
}

const toRepoPath = (full: string): string => relative(REPO_ROOT, full).split(sep).join('/')

/** Caminha a árvore a partir de `from`, devolvendo caminhos relativos à raiz do repo com `/`. */
export function walk(from: string, keep: (relPath: string) => boolean): string[] {
  const found: string[] = []
  const stack = [resolve(REPO_ROOT, from)]

  while (stack.length > 0) {
    const dir = stack.pop()
    if (dir === undefined) continue

    for (const entry of entriesOf(dir)) {
      const full = join(dir, entry.name)
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name)) stack.push(full)
      } else if (keep(toRepoPath(full))) {
        found.push(toRepoPath(full))
      }
    }
  }

  return found.sort()
}

// Descobre `apps/<app>/src` e `packages/<pkg>/src` em disco em vez de listar à mão — um pacote
// novo entrava em silêncio fora de doze gates quando a lista era estática.
export function sourceRoots(): string[] {
  const roots: string[] = []
  for (const group of ['apps', 'packages']) {
    for (const entry of entriesOf(resolve(REPO_ROOT, group))) {
      if (!entry.isDirectory() || SKIP_DIRS.has(entry.name)) continue
      const rel = `${group}/${entry.name}/src`
      if (exists(rel)) roots.push(rel)
    }
  }
  return roots.sort()
}

/** `package.json` de todo workspace com fonte — mesma razão de `sourceRoots`. */
export function workspaceManifests(): string[] {
  return sourceRoots()
    .map((root) => root.replace(/\/src$/, '/package.json'))
    .filter(exists)
}

export function exists(relPath: string): boolean {
  try {
    statSync(resolve(REPO_ROOT, relPath))
    return true
  } catch {
    return false
  }
}

export function read(relPath: string): string {
  return readFileSync(resolve(REPO_ROOT, relPath), 'utf8')
}

export const isSource = (rel: string): boolean => /\.(ts|tsx|mts|mjs)$/.test(rel) && !rel.endsWith('.d.ts')

export const isTest = (rel: string): boolean => rel.includes('/__tests__/') || /\.(test|spec)\.[cm]?tsx?$/.test(rel)

/**
 * Todo campo declarado em `apps/desktop/src-tauri/src/config/schemas/*.rs` — `${namespace}.${key}`
 * → arquivo de origem. Compartilhado por `check:config-schema-parity` e `check:config-read-paths`,
 * que fazem as duas metades da mesma pergunta (schema → UI, chamador → schema).
 *
 * Um comentário de doc que EXPLICA o formato `name: "X", fields: vec![...]` casa com o próprio
 * regex — já aconteceu neste diretório. Por isso toda linha `//`/`///` é descartada antes de ler.
 *
 * O fim do `vec![...]` é achado por CONTAGEM de colchete, não por "`]` isolado em linha própria":
 * `character.rs` tem `.options(&[...])` aninhado dentro do próprio campo, e o `]),` desse aninhado
 * também cai isolado em linha própria — um não-guloso que parasse no primeiro `]` desse tipo cortava
 * o namespace `character` na metade e perdia `proactiveness`, `quietHoursStart` e mais 15 campos,
 * sem erro nenhum (o total só ficava mais baixo).
 *
 * `voice.rs` não declara `fields: vec![...]` inline — é `Namespace { name: "voice", fields }`
 * (abreviação de struct), com `fields` montado por `runtime_fields().extend(elevenlabs_fields())…`
 * espalhado em oito funções auxiliares do arquivo. Sem o fallback abaixo, `voice` inteiro (56 campos)
 * ficava invisível: nem "sem UI" nem "sem chamador" — zero linha do arquivo virava violação, porque
 * zero campo era reconhecido. Como o arquivo só declara UM namespace, todo `FIELD_KEY` do arquivo
 * pertence a ele.
 */
export function configSchemaLeafPaths(): Map<string, string> {
  const fields = new Map<string, string>()
  for (const file of walk(SCHEMA_ROOT, (rel) => rel.endsWith('.rs'))) {
    for (const [namespace, body] of schemaNamespaceBodies(stripLineComments(read(file)))) {
      for (const field of body.matchAll(FIELD_KEY)) {
        const path = `${namespace}.${field[1]}`
        if (!fields.has(path)) fields.set(path, file)
      }
    }
  }
  return fields
}

const SCHEMA_ROOT = 'apps/desktop/src-tauri/src/config/schemas'
const NAMESPACE_HEAD = /name:\s*"([^"]+)",\s*fields:\s*vec!\[/g
const NAMESPACE_SHORTHAND = /name:\s*"([^"]+)",\s*fields\s*\}/
const FIELD_KEY = /\b(?:boolean|int_range|int|float_range|float|text|choice|list|map)\(\s*"([^"]+)"/g

const stripLineComments = (source: string): string =>
  source
    .split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n')

/**
 * Pares `[namespace, trecho onde procurar campos]`. Sem nenhum `fields: vec![...]` inline, a
 * abreviação `name: "X", fields }` declara um único namespace com campos montados fora do literal —
 * vale o arquivo inteiro.
 */
function schemaNamespaceBodies(source: string): Array<[string, string]> {
  const inline = [...source.matchAll(NAMESPACE_HEAD)].map((head): [string, string] => {
    const bodyStart = (head.index ?? 0) + head[0].length
    return [head[1] as string, source.slice(bodyStart, matchingBracketEnd(source, bodyStart))]
  })
  if (inline.length > 0) return inline
  const shorthand = NAMESPACE_SHORTHAND.exec(source)
  return shorthand === null ? [] : [[shorthand[1] as string, source]]
}

/** Índice do `]` que fecha o `[` aberto logo antes de `bodyStart`, contando aninhamento. */
function matchingBracketEnd(source: string, bodyStart: number): number {
  let depth = 1
  for (let i = bodyStart; i < source.length; i++) {
    if (source[i] === '[') depth++
    else if (source[i] === ']' && --depth === 0) return i
  }
  return source.length
}

export type Violation = { file: string; line: number; message: string }

/** Localiza a linha 1-indexed de um índice de caractere. */
export function lineOf(source: string, index: number): number {
  let line = 1
  for (let i = 0; i < index && i < source.length; i++) {
    if (source[i] === '\n') line++
  }
  return line
}

/**
 * Encerra o gate. Saída 1 com a lista de violações, saída 0 com uma linha de resumo.
 * A saída é curta de propósito: um gate que despeja o repositório inteiro não é lido.
 */
export function report(gate: string, violations: Violation[], checked: number): never {
  if (violations.length === 0) {
    process.stdout.write(`✓ ${gate} — ${checked} arquivo(s) conferido(s)\n`)
    process.exit(0)
  }

  process.stdout.write(`✗ ${gate} — ${violations.length} violação(ões)\n`)
  for (const v of violations.slice(0, 60)) {
    process.stdout.write(`  ${v.file}:${v.line}  ${v.message}\n`)
  }
  if (violations.length > 60) {
    process.stdout.write(`  … e mais ${violations.length - 60}\n`)
  }
  process.exit(1)
}
