/**
 * check:external-links — link externo na UI sai por `open-external` (`lib/shell-links.ts`), nunca
 * por `<a target="_blank">` nem `window.open`.
 *
 * Existe porque a classe de defeito estava em produção depois da migração Tauri (2026-09-28): o
 * "Instalar Ollama" do wizard, o "Saiba mais" do AuroraField (usado no "Obter chave" dos templates
 * MCP) e o "Documentação" dos campos de settings eram `<a href target="_blank">`. O webview do Tauri
 * descarta pedido de janela nova sem erro e sem log — o link parece vivo e não faz nada. O caminho
 * que funciona é a ação `openExternal` do shell, cuja allowlist de hosts o teste Rust
 * `every_link_the_ui_opens_is_on_the_open_external_allowlist` confere contra os links declarados.
 */
import { isSource, isTest, lineOf, read, report, type Violation, walk } from './lib.mts'

const ROOTS = ['apps/desktop/src/renderer/src', 'apps/desktop/src/features', 'packages/sdk-ui/src']

const RULES: { pattern: RegExp; message: string }[] = [
  {
    pattern: /target=["'{`]+_blank/g,
    message: 'target="_blank" — o webview do Tauri descarta a janela nova em silêncio; use openExternalLink',
  },
  { pattern: /\bwindow\.open\(/g, message: 'window.open — morto no Tauri; use openExternalLink de lib/shell-links.ts' },
  { pattern: /href=\{?["'`]https?:/g, message: 'href externo — clicar navega o próprio webview; use openExternalLink' },
]

const files = ROOTS.flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel)))
const violations: Violation[] = []

for (const file of files) {
  const source = read(file)
  for (const rule of RULES) {
    for (const match of source.matchAll(rule.pattern)) {
      violations.push({ file, line: lineOf(source, match.index ?? 0), message: rule.message })
    }
  }
}

// Piso de alcance: raiz errada que varra zero arquivo não pode sair verde.
if (files.length < 100) {
  violations.push({
    file: 'scripts/checks/external-links.mts',
    line: 1,
    message: `varreu só ${files.length} arquivo(s) — piso de alcance quebrado, confira ROOTS`,
  })
}

report('check:external-links', violations, files.length)
