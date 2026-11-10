/**
 * check:ui-controls — pega controle de formulário cru (select/input/checkbox/range/switch) fora do
 * design system, exigido por AGENTS.md §3.6. Ignora `packages/sdk-ui` (dono desses elementos).
 * Exceção só entra na ALLOWLIST com arquivo + motivo.
 */
import { isSource, isTest, lineOf, read, report, type Violation, walk } from './lib.mts'

const ROOTS = ['apps/desktop/src/renderer/src', 'apps/desktop/src/features', 'apps/desktop/src/main/services']

const FEATURE_UI = /^apps\/desktop\/src\/features\/[^/]+\/renderer\//
const SERVICE_UI = /^apps\/desktop\/src\/main\/services\/[^/]+\/renderer\//
const SHELL_UI = /^apps\/desktop\/src\/renderer\/src\//

function inScope(rel: string): boolean {
  if (!isSource(rel) || isTest(rel)) return false
  if (rel.startsWith('packages/sdk-ui/')) return false
  return SHELL_UI.test(rel) || FEATURE_UI.test(rel) || SERVICE_UI.test(rel)
}

// arquivo → quantos controles crus pode ter e por quê (quantidade, não linha, que muda a cada edição).
const ALLOWLIST: Record<string, { count: number; reason: string }> = {
  'apps/desktop/src/renderer/src/lib/window-drag.ts': {
    count: 1,
    reason: 'seletor CSS que EXCLUI controles do arrasto da janela — não renderiza controle nenhum',
  },
  'apps/desktop/src/renderer/src/components/settings/SettingsCommandPalette.tsx': {
    count: 1,
    reason: 'busca de texto livre da paleta de comandos de settings — não é campo de configuração',
  },
  'apps/desktop/src/renderer/src/components/CommandPalette.tsx': {
    count: 1,
    reason: 'busca de texto livre da paleta de comandos global — não é campo de configuração',
  },
  'apps/desktop/src/renderer/src/components/chat-history-search-bar.tsx': {
    count: 1,
    reason: 'busca de texto livre no histórico de chat — não é campo de configuração',
  },
  'apps/desktop/src/renderer/src/components/app/chat-composer-subcomponents.tsx': {
    count: 1,
    reason: 'composer de chat — texto livre da conversa, não um campo de configuração',
  },
  'apps/desktop/src/renderer/src/components/settings/BackupPanel.tsx': {
    count: 1,
    reason: 'seletor de arquivo .json para importar configurações — não é campo de configuração',
  },
}

type Rule = { pattern: RegExp; message: string }

const RULES: Rule[] = [
  { pattern: /<select\b/g, message: 'elemento <select> cru — use GlassSelect de @mika/sdk-ui/settings' },
  {
    pattern: /<input\b/g,
    message: 'elemento <input> cru — use AuroraField (ou GlassSlider para range) de @mika/sdk-ui/settings',
  },
  {
    pattern: /<textarea\b/g,
    message: 'elemento <textarea> cru — use AuroraField (multiline) de @mika/sdk-ui/settings',
  },
  {
    pattern: /type=["']checkbox["']/g,
    message: 'input type="checkbox" cru — use GlassToggle de @mika/sdk-ui/settings',
  },
  { pattern: /type=["']range["']/g, message: 'input type="range" cru — use GlassSlider de @mika/sdk-ui/settings' },
  {
    pattern: /role=["']switch["']/g,
    message: 'role="switch" reimplementado à mão — use GlassToggle de @mika/sdk-ui/settings',
  },
]

const files = ROOTS.flatMap((root) => walk(root, inScope))

const violations: Violation[] = []

for (const file of files) {
  const source = read(file)
  let allowance = ALLOWLIST[file]?.count ?? 0
  for (const rule of RULES) {
    for (const match of source.matchAll(rule.pattern)) {
      if (allowance-- > 0) continue
      violations.push({ file, line: lineOf(source, match.index ?? 0), message: rule.message })
    }
  }
}

// Piso de alcance: um filtro de raiz errado que passe a varrer zero arquivo não deve sair verde.
if (files.length < 100) {
  violations.push({
    file: 'scripts/checks/ui-controls.mts',
    line: 1,
    message: `varreu só ${files.length} arquivo(s) — piso de alcance quebrado, confira ROOTS/inScope antes de confiar no resultado`,
  })
}

report('check:ui-controls', violations, files.length)
