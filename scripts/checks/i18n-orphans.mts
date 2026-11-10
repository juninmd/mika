/**
 * check:i18n-orphans — chave do dicionário que nenhum código cita. Cada tela enxugada deixava suas
 * chaves para trás (133 numa só varredura), e cada uma custa 10 traduções mantidas para nada.
 * "Cita" = o literal da chave entre aspas em TS/TSX/Rust/JSON, ou um prefixo montado em runtime
 * (`t(\`a.b.${x}\`)`, `'a.b.' + x`), que cobre toda chave que começa por ele.
 */
import { lineOf, read, report, type Violation, walk } from './lib.mts'

const DICT = 'packages/sdk/src/i18n/translation-dict.ts'
const ROOTS = ['apps/desktop/src', 'apps/desktop/src-tauri/src', 'packages']
const keep = (rel: string): boolean =>
  /\.(ts|tsx|mts|rs|json)$/.test(rel) && !rel.endsWith('.d.ts') && !rel.startsWith('packages/sdk/src/i18n/')

const dict = read(DICT)
const keys = [...dict.matchAll(/^\s*'([^']+)':\s*string/gm)].map((m) => ({ key: m[1] ?? '', at: m.index ?? 0 }))
const files = ROOTS.flatMap((root) => walk(root, keep))
const corpus = files.map(read).join('\n')
const prefixes = [...corpus.matchAll(/[`'"]([a-zA-Z][\w-]*(?:\.[\w-]+)*\.)(?:\$\{|['"`]\s*\+)/g)].map((m) => m[1] ?? '')
const quoted = (key: string): boolean => [`'${key}'`, `"${key}"`, `\`${key}\``].some((q) => corpus.includes(q))

const violations: Violation[] = keys
  .filter(({ key }) => !quoted(key) && !prefixes.some((p) => key.startsWith(p)))
  .map(({ key, at }) => ({
    file: DICT,
    line: lineOf(dict, at),
    message: `chave "${key}" sem uso — apague do dicionário e dos 10 locales`,
  }))

if (keys.length < 1000 || files.length < 500) {
  violations.push({
    file: 'scripts/checks/i18n-orphans.mts',
    line: 1,
    message: `leu ${keys.length} chave(s) e ${files.length} arquivo(s) — piso de alcance quebrado, confira DICT/ROOTS`,
  })
}

report('check:i18n-orphans', violations, files.length)
