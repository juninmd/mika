/**
 * check:ui-i18n — pega texto de interface escrito direto no código em vez de `t()`. A troca de
 * idioma só alcança o que passa pelo dicionário; string fixa fica em português em qualquer locale, e
 * a varredura por screenshot em en-US/ja-JP achou ~120 delas espalhadas por 30 arquivos.
 * Heurística: texto com maiúscula inicial entre tags, em prop JSX de texto (aspas ou template),
 * em campo de objeto de texto (`label: '...'`) — este pega a cópia que mora em `.ts` (listas,
 * presets, specs) —, em braço de ternário (`ok ? 'Salvo' : ...`), em `setError(...)`, em
 * `throw new Error('...')` (a mensagem vira o banner de erro da tela), em `alt=`, nos valores de um
 * `Record<string, string> = { ... }` (tabela de rótulos: `TOOL_LABELS`, `BUILT_IN_TAB_LABELS` moravam
 * assim e escapavam de todos os padrões acima), em `return 'Texto'` e em `set…Status('Texto')`
 * (a barra de status do teste de VAD era português fixo em todo locale).
 * `packages/sdk-ui/src` entra na varredura: os controles compartilhados também desenham texto.
 */
import { isTest, lineOf, read, report, type Violation, walk } from './lib.mts'

const ROOTS = ['apps/desktop/src/renderer', 'apps/desktop/src/features', 'packages/sdk-ui/src']
const FEATURE_UI = /^apps\/desktop\/src\/features\/[^/]+\/renderer\//

function inScope(rel: string): boolean {
  if (!/\.tsx?$/.test(rel) || rel.endsWith('.d.ts') || isTest(rel)) return false
  // O copy do wizard é o próprio dicionário por idioma; o manifesto de módulo é contrato, não tela.
  if (rel.includes('/wizard/copy/') || rel.endsWith('.module.ts')) return false
  return !rel.startsWith('apps/desktop/src/features/') || FEATURE_UI.test(rel)
}

// arquivo → quantos textos fixos pode ter e por quê.
const ALLOWLIST: Record<string, { count: number; reason: string }> = {
  'apps/desktop/src/renderer/src/components/app/ShellFallbacks.tsx': { count: 1, reason: 'nome do produto' },
  'apps/desktop/src/features/character/renderer/character-identity-panel.tsx': {
    count: 1,
    reason: 'nome da personagem',
  },
  'apps/desktop/src/features/gamer-companion/renderer/widgets/steam-library.widget.tsx': { count: 1, reason: 'marca' },
  'apps/desktop/src/features/person-catalog/renderer/views/person-catalog-listing.view.tsx': {
    count: 1,
    reason: 'nome da personagem',
  },
  'apps/desktop/src/features/user/renderer/settings.tsx': { count: 2, reason: 'formato de data e id IANA de fuso' },
  'apps/desktop/src/renderer/settings/llm/embedding-provider-catalog.ts': { count: 5, reason: 'nomes de provedor' },
  'apps/desktop/src/renderer/settings/llm/llm-providers.ts': { count: 5, reason: 'nomes de provedor' },
  'apps/desktop/src/renderer/settings/llm/openrouter-model-panel.tsx': { count: 1, reason: 'marca' },
  'apps/desktop/src/renderer/src/components/command-palette-static-items.ts': {
    count: 3,
    reason: 'atalhos de teclado',
  },
  'apps/desktop/src/renderer/src/components/live2d/model-path.ts': { count: 1, reason: 'nome do modelo' },
  'apps/desktop/src/renderer/src/components/settings/provider-credential-specs.ts': {
    count: 2,
    reason: 'prefixo de formato da chave',
  },
  'apps/desktop/src/renderer/src/components/settings/settings-utils.ts': {
    count: 2,
    reason: 'nome de provedor e chave de agrupamento (`General`, identificador e não texto)',
  },
  'apps/desktop/src/renderer/src/components/settings/skin/SkinPanel.tsx': {
    count: 1,
    reason: 'nome da tecnologia (Live2D)',
  },
  'apps/desktop/src/renderer/src/components/use-neural-wheel-items.tsx': { count: 1, reason: 'marca (Steam)' },
  'apps/desktop/src/renderer/src/hooks/useAudioPlayback.ts': {
    count: 1,
    reason: 'invariante interna: o erro só chega ao log, nunca à tela',
  },
  'apps/desktop/src/renderer/src/lib/app-shortcuts.ts': { count: 1, reason: 'nome da tecla (Ctrl)' },
  'apps/desktop/src/renderer/src/lib/keyboard-shortcuts.ts': { count: 1, reason: 'nome da tecla (Ctrl)' },
  'apps/desktop/src/renderer/src/hooks/useMikaMenuItems.tsx': { count: 1, reason: 'atalho de teclado' },
}

// Uma palavra de 4+ letras com maiúscula já é cópia; pontuação (`Carregando…`) não a esconde.
const JSX_TEXT = />\s*([A-ZÀ-Ú][^<>{}\n]*[a-zà-ú]{3}[^<>{}\n]*?)\s*</g
const TEXT_PROPS = 'label|title|placeholder|aria-label|description|confirmLabel|hint|alt'
const TEXT_PROP = new RegExp(`\\b(?:${TEXT_PROPS})=["']([A-ZÀ-Ú][^"'{}]{3,})["']`, 'g')
// `aria-label={`Abrir ${nome}`}` — o texto fixo mora no template, e o `${}` não o esconde.
const TEMPLATE_PROP = new RegExp(`\\b(?:${TEXT_PROPS})=\\{\`([A-ZÀ-Ú][^\`$]{3,})`, 'g')
const OBJECT_PROP =
  /\b(?:label|title|placeholder|description|hint|subtitle|subLabel|detail|tooltip):\s*['"`]([A-ZÀ-Ú][^'"`$]{3,})['"`]/g
// `cond ? 'Salvo' : 'Salvar'` — os dois braços do ternário, com maiúscula e uma palavra de 3+ letras.
const TERNARY_THEN = /\?\s*['"`]([A-ZÀ-Ú][a-zà-ú][^'"`$\n]{2,})['"`]/g
const TERNARY_ELSE = /\?\s*['"`][^'"`\n]*['"`]\s*:\s*['"`]([A-ZÀ-Ú][a-zà-ú][^'"`$\n]{2,})['"`]/g
// `addNotification('info', 'Texto', 'Título')` — a notificação é a tela falando com a pessoa.
const NOTIFICATION = /\baddNotification\(\s*['"`]\w+['"`]\s*,\s*['"`]([A-ZÀ-Ú][^'"`$]{3,})['"`]/g
// `err.message ?? 'Falha'` / `toDisplayError(err, 'Falha ao salvar')` — o texto que sobra quando o erro não traz mensagem.
const FALLBACK_TEXT = /(?:[?|]{2}|\btoDisplayError\([^,()]+,)\s*['"`]([A-ZÀ-Ú][a-zà-ú][^'"`$\n]{2,})['"`]/g
// `setVadStatus('Escuta iniciada.')` / `setStatus(`Erro: ${x}`)` — a linha de status é texto de tela.
const SET_STATUS = /\bset\w*Status\(\s*['"`]([A-ZÀ-Ú][a-zà-ú][^'"`$\n]{2,})/g
// `return 'Texto fixo'` — função que devolve a copy. Duas palavras ou pontuação final distinguem frase de
// identificador (`return 'Idle'`, `return 'Escape'`).
const RETURN_TEXT = /\breturn\s+['"`]([A-ZÀ-Ú][a-zà-ú]+(?:\s+[\wà-ú]+[^'"`$\n]*|[^'"`$\n]*[.!?…:]))['"`]/g
const RECORD_OPEN = /\bRecord<string,\s*string>\s*=\s*\{/g
const RECORD_VALUE = /^\s*['"]?[\w-]+['"]?:\s*['"`]([A-ZÀ-Ú][a-zà-ú][^'"`$\n]{2,})['"`]/gm
const SET_ERROR = /\bsetError\(\s*['"`]([A-ZÀ-Ú][^'"`$]{3,})['"`]/g
const THROW_ERROR = /\bthrow new Error\(\s*['"`]([A-ZÀ-Ú][^'"`$]{3,})['"`]/g

// `=> Promise<void>` and `Record<K, V>` look like `>Text<` to the JSX pattern; a lone identifier
// opening a generic is a type, never copy.
const isTypeSyntax = (matched: string, next: string): boolean =>
  /[\w{[(]/.test(next) && /^>\s*[A-Z][\w.:\s]*</.test(matched) && !/\s[a-zà-ú]/.test(matched)

/** Índice logo após a chave que fecha o literal aberto em `start`. */
function closingBrace(source: string, start: number): number {
  let depth = 1
  let end = start
  while (end < source.length && depth > 0) {
    const ch = source[end++]
    if (ch === '{') depth++
    else if (ch === '}') depth--
  }
  return end
}

/** Texto fixo nos valores de cada `Record<string, string> = { … }`. */
function recordFindings(source: string): Array<{ index: number; text: string }> {
  return [...source.matchAll(RECORD_OPEN)].flatMap((open) => {
    const start = (open.index ?? 0) + open[0].length
    const body = source.slice(start, closingBrace(source, start))
    return [...body.matchAll(RECORD_VALUE)].map((match) => ({
      index: start + (match.index ?? 0) + match[0].search(/\S/),
      text: match[1] ?? '',
    }))
  })
}

function findings(file: string): Violation[] {
  const source = read(file)
  const found: Violation[] = recordFindings(source).map(({ index, text }) => ({
    file,
    line: lineOf(source, index),
    message: `texto fixo "${text.trim()}" — mova para o dicionário e use t()`,
  }))
  for (const pattern of [
    JSX_TEXT,
    TEXT_PROP,
    TEMPLATE_PROP,
    OBJECT_PROP,
    TERNARY_THEN,
    TERNARY_ELSE,
    NOTIFICATION,
    FALLBACK_TEXT,
    SET_STATUS,
    RETURN_TEXT,
    SET_ERROR,
    THROW_ERROR,
  ]) {
    for (const match of source.matchAll(pattern)) {
      if (pattern === JSX_TEXT && isTypeSyntax(match[0], source[(match.index ?? 0) + match[0].length] ?? '')) continue
      found.push({
        file,
        line: lineOf(source, match.index ?? 0),
        message: `texto fixo "${match[1]?.trim()}" — mova para o dicionário e use t()`,
      })
    }
  }
  return found
}

const files = ROOTS.flatMap((root) => walk(root, inScope))
const violations: Violation[] = []
for (const file of files) {
  const found = findings(file)
  const allowed = ALLOWLIST[file]?.count ?? 0
  violations.push(...found.slice(allowed))
  // Exceção que sobra esconderia o próximo texto fixo do mesmo arquivo.
  if (found.length < allowed) {
    violations.push({
      file,
      line: 1,
      message: `allowlist pede ${allowed}, o arquivo tem ${found.length} — baixe o número`,
    })
  }
}

if (files.length < 400) {
  violations.push({
    file: 'scripts/checks/ui-i18n.mts',
    line: 1,
    message: `varreu só ${files.length} arquivo(s) — piso de alcance quebrado, confira ROOTS/inScope antes de confiar no resultado`,
  })
}

report('check:ui-i18n', violations, files.length)
