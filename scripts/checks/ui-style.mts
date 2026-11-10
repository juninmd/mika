/**
 * check:ui-style — pega os três desvios visuais que a revisão de UI por screenshot achou repetidos:
 * fonte abaixo do piso de legibilidade (`text-[≤11px]` em vez de `text-micro`/`text-caption`), cor
 * crua (rgba/hsl/#hex) que ignora o tema, e `bg-accent` com `text-accent-text` (rosa sobre rosa).
 * Arquivo cuja cor É o dado (paleta de tema, cena, marca de plataforma) entra na ALLOWLIST com motivo.
 */
import { isSource, isTest, lineOf, read, report, type Violation, walk } from './lib.mts'

const ROOTS = ['apps/desktop/src/renderer', 'apps/desktop/src/features', 'packages/sdk-ui/src']
const FEATURE_UI = /^apps\/desktop\/src\/features\/[^/]+\/renderer\//

function inScope(rel: string): boolean {
  if (!isSource(rel) || isTest(rel)) return false
  return !rel.startsWith('apps/desktop/src/features/') || FEATURE_UI.test(rel)
}

// arquivo → quantas cores cruas pode ter e por quê (quantidade, não linha, que muda a cada edição).
const RAW_COLOR_ALLOWLIST: Record<string, { count: number; reason: string }> = {
  'apps/desktop/src/renderer/src/themes.ts': { count: 56, reason: 'paleta dos temas — é a origem dos tokens' },
  'apps/desktop/src/renderer/src/themes-extra.ts': { count: 70, reason: 'paleta dos temas extras' },
  'packages/sdk-ui/src/utils/accent.ts': { count: 5, reason: 'deriva os tokens de acento a partir do hue' },
  'apps/desktop/src/renderer/src/components/avatar/vrm-look.ts': {
    count: 62,
    reason: 'material do modelo 3D (three.js)',
  },
  'apps/desktop/src/renderer/src/components/avatar/vrm-accessories.ts': { count: 14, reason: 'material do modelo 3D' },
  'apps/desktop/src/renderer/src/components/avatar/vrm-blush.ts': { count: 3, reason: 'tinta de pele do modelo 3D' },
  'apps/desktop/src/renderer/src/components/avatar/vrm-props.ts': {
    count: 15,
    reason: 'material dos props 3D (garrafa, tomate)',
  },
  'apps/desktop/src/renderer/src/components/avatar/vrm-postfx.ts': {
    count: 2,
    reason: 'pós-processamento do canvas 3D',
  },
  'apps/desktop/src/renderer/src/components/effects/scenes-ambient.ts': {
    count: 5,
    reason: 'partículas desenhadas em canvas',
  },
  'apps/desktop/src/renderer/src/components/effects/scenes-digital.ts': {
    count: 4,
    reason: 'partículas desenhadas em canvas',
  },
  'apps/desktop/src/renderer/src/components/effects/scene-fireworks.ts': {
    count: 2,
    reason: 'partículas desenhadas em canvas',
  },
  'apps/desktop/src/features/gamer-companion/renderer/widgets/truco-deck.ts': { count: 4, reason: 'naipes do baralho' },
  'apps/desktop/src/renderer/src/components/settings/MermaidDiagram.tsx': {
    count: 6,
    reason: 'themeVariables do mermaid exigem cor resolvida, não var(--x)',
  },
  'apps/desktop/src/renderer/src/components/settings/plugin-visuals.tsx': {
    count: 5,
    reason: 'cada plugin tem hue próprio, exposto como --plugin-accent-*',
  },
  'packages/sdk-ui/src/components/FeatureCard.tsx': { count: 3, reason: 'API pública recebe accentHue' },
  'packages/sdk-ui/src/components/GlassCard.tsx': { count: 2, reason: 'sombra escura (slate), não cor de tema' },
  'packages/sdk-ui/src/components/GlassSection.tsx': { count: 1, reason: 'sombra escura (slate), não cor de tema' },
  'apps/desktop/src/features/character/renderer/character-identity-config.ts': {
    count: 1,
    reason: 'valor padrão da config backgroundSolidColor',
  },
  'apps/desktop/src/features/character/renderer/character-identity-panel.tsx': {
    count: 1,
    reason: 'placeholder do campo de cor (formato esperado)',
  },
  'apps/desktop/src/renderer/src/hooks/useShellUiState.ts': {
    count: 1,
    reason: 'valor padrão de backgroundSolidColor',
  },
  'apps/desktop/src/renderer/src/hooks/useDesktopShellBootstrap.ts': {
    count: 1,
    reason: 'valor padrão de backgroundSolidColor',
  },
}

// Fractional px is the same breach: `8.5px` slipped past the integer-only form.
const TINY_TEXT = /text-\[(?:\d|1[01])(?:\.\d+)?px\]/g
const RAW_COLOR = /rgba?\(|hsla?\(|['"`[\s(,]#[0-9a-fA-F]{3,8}\b/g
// Preto é sombra, não cor de tema; `var(--x, #hex)` é fallback de token, não cor solta.
const TOLERATED = /^(?:rgba?\(0[,_\s]+0[,_\s]+0\b)/
const ACCENT_ON_ACCENT =
  /\bbg-accent(?![-\w/])[^'"`\n]*\btext-accent-text\b|\btext-accent-text\b[^'"`\n]*\bbg-accent(?![-\w/])/g

const stripComment = (line: string): string => line.replace(/^\s*(?:\/\/|\*|\/\*).*$/, '').replace(/\s\/\/\s.*$/, '')

function rawColors(file: string, source: string): Violation[] {
  const found: Violation[] = []
  const lines = source.split('\n')
  lines.forEach((line, i) => {
    const code = stripComment(line)
    for (const match of code.matchAll(RAW_COLOR)) {
      const at = match.index ?? 0
      if (TOLERATED.test(code.slice(at))) continue
      if (/var\(--[\w-]+,\s*$/.test(code.slice(0, at + 1))) continue
      found.push({
        file,
        line: i + 1,
        message: 'cor crua — use token do tema (var(--accent), bg-ok, color-mix(in srgb, var(--x) N%, transparent))',
      })
    }
  })
  return found
}

function check(file: string): Violation[] {
  const source = read(file)
  const found: Violation[] = []
  for (const match of source.matchAll(TINY_TEXT)) {
    found.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message: `${match[0]} abaixo do piso — use text-micro (10px, rótulo caixa-alta) ou text-caption (11px)`,
    })
  }
  for (const match of source.matchAll(ACCENT_ON_ACCENT)) {
    found.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message: 'bg-accent + text-accent-text some no contraste — use bg-accent-soft com text-accent-text',
    })
  }
  const colors = rawColors(file, source)
  return [...found, ...colors.slice(RAW_COLOR_ALLOWLIST[file]?.count ?? 0)]
}

const files = ROOTS.flatMap((root) => walk(root, inScope))
const violations = files.flatMap(check)

// Allowlist que sobra é exceção que ninguém mais usa — ela esconderia a próxima cor crua do arquivo.
for (const [file, { count }] of Object.entries(RAW_COLOR_ALLOWLIST)) {
  const used = files.includes(file) ? rawColors(file, read(file)).length : 0
  if (used < count) {
    violations.push({
      file,
      line: 1,
      message: `allowlist pede ${count} cor(es) crua(s), o arquivo tem ${used} — baixe o número`,
    })
  }
}

if (files.length < 200) {
  violations.push({
    file: 'scripts/checks/ui-style.mts',
    line: 1,
    message: `varreu só ${files.length} arquivo(s) — piso de alcance quebrado, confira ROOTS/inScope antes de confiar no resultado`,
  })
}

report('check:ui-style', violations, files.length)
