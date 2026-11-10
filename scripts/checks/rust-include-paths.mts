/**
 * `include_str!` que aponta para arquivo inexistente.
 *
 * **Por que este gate existe.** Metade dos testes de contrato cross-language deste repositório é
 * `include_str!` lendo o fonte TypeScript ou outro módulo Rust para provar que os dois lados não
 * divergiram. O caminho é resolvido pelo compilador — mas quase todos esses `include_str!` vivem
 * dentro de `#[cfg(test)]`, e `cargo check` NÃO compila código de teste. Resultado: mover um
 * arquivo deixa o `cargo check` verde e só quebra 148 segundos depois, no `cargo test`, que
 * recompila a crate inteira e derruba a suíte de TODAS as frentes ao mesmo tempo.
 *
 * Aconteceu duas vezes em 2026-09-21, na mesma rodada: `modules/tools/registry.rs` virou
 * `registry/mod.rs` e `modules/privacy.rs` virou `privacy/mod.rs`. Cada split muda a profundidade
 * relativa de todo `include_str!` do arquivo movido, e quebra os de fora que apontavam para ele.
 *
 * Este gate custa 2 segundos e responde a mesma pergunta que os 148.
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'

const ROOT = resolve(import.meta.dirname, '../..')
const RUST_SRC = join(ROOT, 'apps/desktop/src-tauri/src')

function rustFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = join(dir, entry.name)
    if (entry.isDirectory()) return rustFiles(full)
    return entry.name.endsWith('.rs') ? [full] : []
  })
}

/**
 * Corta a linha no `//` para não ler prosa como código: um comentário que MENCIONA
 * `include_str!` (para explicar por que não se deve usar, por exemplo) não é uma inclusão.
 * Mesma classe do gate que acusava o próprio aviso contra o padrão que ele proibia.
 */
function codeOf(line: string): string {
  const comment = line.indexOf('//')
  return comment === -1 ? line : line.slice(0, comment)
}

const broken: string[] = []
let checked = 0

for (const file of rustFiles(RUST_SRC)) {
  const lines = readFileSync(file, 'utf8').split('\n')
  for (const [index, line] of lines.entries()) {
    // `include_str!(` e `include_bytes!(`, com o literal na mesma linha ou na seguinte —
    // `rustfmt` quebra a linha quando o caminho é longo, que é o caso dos que sobem até o TS.
    const macroCall = /\b(include_str|include_bytes)!\s*\(/.exec(codeOf(line))
    if (!macroCall) continue
    // O literal tem de vir DEPOIS do `include_str!(` — uma tupla como `("mood", include_str!(...))`
    // tem outra string entre aspas ANTES da macro na mesma linha, e pegar "a primeira aspas da
    // linha" acusava essa string como se fosse o caminho.
    const afterMacro = codeOf(line).slice(macroCall.index + macroCall[0].length)
    const literal = /"([^"]+)"/.exec(afterMacro) ?? /"([^"]+)"/.exec(codeOf(lines[index + 1] ?? ''))
    if (!literal) continue

    checked += 1
    const target = resolve(dirname(file), literal[1])
    if (!existsSync(target) || !statSync(target).isFile()) {
      broken.push(
        `  ${relative(ROOT, file).replaceAll('\\', '/')}:${index + 1}\n` +
          `    aponta para "${literal[1]}" → ${relative(ROOT, target).replaceAll('\\', '/')} (não existe)`,
      )
    }
  }
}

if (broken.length > 0) {
  console.error(
    `✗ check:rust-include-paths — ${broken.length} de ${checked} include_str!/include_bytes! apontam para arquivo inexistente:\n` +
      `${broken.join('\n')}\n\n` +
      `  Alguém moveu ou dividiu um arquivo. Ajuste o caminho (um split adiciona UM nível de\n` +
      `  profundidade) e prefira o arquivo que DECLARA o que o teste afirma, não o de teste.`,
  )
  process.exit(1)
}

console.log(`✓ check:rust-include-paths — ${checked} include_str!/include_bytes! conferidos, todos resolvem`)
