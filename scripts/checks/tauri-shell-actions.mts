/**
 * check:tauri-shell-actions — toda ação pedida ao shell tem de ter braço em `shell_actions.rs`,
 * e todo braço declarado lá tem de ter chamador.
 *
 * Existe porque a classe de defeito já aconteceu nesta branch, duas vezes: os canais de shell
 * foram escritos no Rust com um prefixo `shell:` que o contrato TypeScript não tem, e nada
 * falhou — mover, fechar e fixar a janela simplesmente não aconteciam, sem erro e sem log.
 * Uma ação sem braço do outro lado é um controle que parece ligado e não faz nada, que é a
 * classe de defeito mais cara deste repositório.
 *
 * Direção inversa também: braço órfão no Rust é código que ninguém chama fingindo capacidade.
 *
 * **O lado chamador mudou de língua em 2026-09-22.** Até então quem pedia era o sidecar Node
 * (`apps/desktop/src/sidecar`) através do shim de Electron (`apps/desktop/src/shared/tauri-main`),
 * e o gate lia esses dois diretórios. Com o sidecar apagado, o primeiro deixou de existir e o
 * segundo virou código sem processo que o execute — continuar lendo os dois manteria VERDE
 * um braço cujo único chamador é um arquivo morto, que é exatamente a leitura que este gate
 * existe para impedir (medir texto, não decisão). Hoje o chamador é Rust: `bridge.rs` traduz o
 * canal do renderer, `decide_boot` decide o boot, a bandeja dispara o resto.
 */
import { lineOf, read, report, type Violation, walk } from './lib.mts'

const RUST = 'apps/desktop/src-tauri/src/shell_actions.rs'

/** Onde o Rust pede ação de shell. */
const CALLER_ROOT = 'apps/desktop/src-tauri/src'

const violations: Violation[] = []

const rust = read(RUST)

/** Braços do `match action` — `"nome" =>`. */
const rustActions = new Set([...rust.matchAll(/^\s{8}"([a-zA-Z]\w*)"\s*(?:\||=>)/gm)].map((m) => m[1]))

if (rustActions.size === 0) {
  violations.push({
    file: RUST,
    line: 1,
    message: 'nenhum braço de ação encontrado — o gate deixou de enxergar o match e passaria verde à toa',
  })
}

const calledActions = new Map<string, { file: string; line: number }>()

for (const file of walk(CALLER_ROOT, (rel) => rel.endsWith('.rs'))) {
  const source = read(file)

  /**
   * `handle(app, "openPluginView", …)` em qualquer módulo, e a forma indireta de `decide_boot`
   * (`let action = if pendente { "openWizard" } else { "revealMain" }`), onde o literal não
   * está colado ao parêntese. Por isso a segunda regra: literal de ação passado a `handle`
   * por variável ainda conta, desde que o arquivo chame `handle` de fato.
   */
  for (const match of source.matchAll(/\bhandle\s*\(\s*&?\w+\s*,\s*"(\w+)"/g)) {
    if (!calledActions.has(match[1])) calledActions.set(match[1], { file, line: lineOf(source, match.index ?? 0) })
  }

  if (!/\bhandle\s*\(\s*&?\w+\s*,\s*\w+\s*,/.test(source)) continue
  for (const match of source.matchAll(/"([a-zA-Z]\w*)"/g)) {
    if (rustActions.has(match[1]) && !calledActions.has(match[1])) {
      calledActions.set(match[1], { file, line: lineOf(source, match.index ?? 0) })
    }
  }
}

for (const [action, where] of calledActions) {
  if (rustActions.has(action)) continue
  violations.push({
    file: where.file,
    line: where.line,
    message: `ação de shell "${action}" não tem braço em ${RUST} — cai no erro de ação desconhecida e o controle fica morto`,
  })
}

for (const action of rustActions) {
  if (calledActions.has(action)) continue
  violations.push({
    file: RUST,
    line: lineOf(rust, rust.indexOf(`"${action}"`)),
    message: `braço "${action}" não é chamado por ninguém em ${CALLER_ROOT} — remova; capacidade sem chamador é controle morto, e a lacuna vai para docs/tauri-rust-migration.md §5`,
  })
}

report('check:tauri-shell-actions', violations, rustActions.size + calledActions.size)
