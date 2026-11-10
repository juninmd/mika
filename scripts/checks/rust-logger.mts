/**
 * check:rust-logger — o shell Tauri usa `log::error!`/`warn!`? Então tem de REGISTRAR um logger.
 *
 * O crate `log` do Rust não tem destino padrão: sem um logger instalado, toda macro vira no-op e
 * a mensagem é descartada em silêncio. Não é teoria — foi assim que o shell rodou por toda a
 * migração: 21 chamadas de log, nenhum logger, e o app subiu com a janela escondida e o backend
 * fora do ar sem imprimir uma única linha. "backend não subiu: {error}" estava escrito no código
 * e nunca podia ser lido por ninguém.
 *
 * Nenhum gate podia ver isso: compila, tipa, passa no `cargo test`. Só executar o app mostrou —
 * e foi preciso ir procurar o log para descobrir que log nenhum existia.
 */

import { readdirSync, readFileSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'

const ROOT = resolve(import.meta.dirname, '../..')
const SHELL_SRC = join(ROOT, 'apps/desktop/src-tauri/src')
const ENTRY = join(SHELL_SRC, 'lib.rs')

type Violation = { file: string; line: number; message: string }
const violations: Violation[] = []

function rustFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = join(dir, entry.name)
    if (entry.isDirectory()) return rustFiles(full)
    return entry.name.endsWith('.rs') ? [full] : []
  })
}

const files = rustFiles(SHELL_SRC)
const callers = files.filter((file) =>
  /(?<![A-Za-z0-9_])log::(error|warn|info|debug|trace)!/.test(readFileSync(file, 'utf8')),
)

const entry = readFileSync(ENTRY, 'utf8')
const registersLogger = /tauri_plugin_log::Builder::new\(\)/.test(entry)

if (callers.length > 0 && !registersLogger) {
  violations.push({
    file: relative(ROOT, ENTRY).replaceAll('\\', '/'),
    line: 1,
    message:
      `${callers.length} arquivo(s) do shell chamam log::… e nenhum logger é registrado em lib.rs — ` +
      'o crate `log` DESCARTA a mensagem sem logger, então todo diagnóstico do shell é código morto. ' +
      'Registre `tauri_plugin_log::Builder::new()` como primeiro plugin.',
  })
}

// `Builder::new()` já traz Stdout + LogDir: `.target()` SOMA a eles. Sem `clear_targets()` cada
// linha saía duas vezes no terminal e o `Mika.log` do LogDir gravava tudo sem a redação de segredo.
if (registersLogger && !/tauri_plugin_log::Builder::new\(\)(\s*\/\/[^\n]*)*\s*\.clear_targets\(\)/.test(entry)) {
  violations.push({
    file: relative(ROOT, ENTRY).replaceAll('\\', '/'),
    line: 1,
    message:
      '`tauri_plugin_log::Builder::new()` sem `.clear_targets()` logo em seguida — os alvos padrão ' +
      '(Stdout + LogDir) continuam ativos: log duplicado no terminal e arquivo sem redação de segredo.',
  })
}

// Piso de alcance: se o gate deixar de enxergar as chamadas, ele passa verde à toa.
if (callers.length === 0) {
  violations.push({
    file: relative(ROOT, SHELL_SRC).replaceAll('\\', '/'),
    line: 1,
    message: 'nenhuma chamada log::… encontrada no shell — o gate deixou de enxergar o padrão e passaria verde à toa',
  })
}

if (violations.length > 0) {
  console.error(`✗ check:rust-logger — ${violations.length} violação(ões)`)
  for (const violation of violations) console.error(`  ${violation.file}:${violation.line}  ${violation.message}`)
  process.exit(1)
}

console.log(`✓ check:rust-logger — ${files.length} arquivo(s) conferido(s), ${callers.length} com log::`)
