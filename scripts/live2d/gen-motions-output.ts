import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { ACTION_DEFS, ACTION_NAMES, UNOBSERVED_ALLOWLIST } from './action-defs'
import { availableParams, buildAction, type GeneratedAction } from './gen-motions-resolve'
import { ACTION_GROUP, type ModelAnalysis } from './model-analysis'

export const MOTION_DIR = 'motion'

export interface ModelResult {
  analysis: ModelAnalysis
  actions: GeneratedAction[]
  notes: string[]
  errors: string[]
}

export function generateForModel(analysis: ModelAnalysis, filePrefix: string): ModelResult {
  const available = availableParams(analysis)
  const physicsSkipped = new Set<string>()
  const allowedUnobserved = new Set(UNOBSERVED_ALLOWLIST[analysis.name] ?? [])
  const actions: GeneratedAction[] = []
  const notes: string[] = []
  const errors: string[] = []

  for (const name of ACTION_NAMES) {
    const action = buildAction(analysis, name, available, allowedUnobserved, physicsSkipped, notes, errors, filePrefix)
    if (action) actions.push(action)
  }

  if (physicsSkipped.size > 0) {
    notes.push(`canais dirigidos pela física, ignorados: ${[...physicsSkipped].sort().join(', ')}`)
  }

  return { analysis, actions, notes, errors }
}

function serialize(value: unknown, compact = false): string {
  return `${JSON.stringify(value, undefined, compact ? undefined : '\t')}\n`
}

export interface PendingWrite {
  path: string
  content: string
}

export function collectWrites(result: ModelResult): PendingWrite[] {
  const { analysis, actions } = result
  const writes: PendingWrite[] = []

  for (const action of actions) {
    writes.push({
      path: join(analysis.dir, MOTION_DIR, action.fileName),
      content: serialize(action.motion, true),
    })
  }

  const model3 = JSON.parse(readFileSync(analysis.model3Path, 'utf8')) as {
    FileReferences?: { Motions?: Record<string, unknown> }
  }
  model3.FileReferences ??= {}
  model3.FileReferences.Motions ??= {}

  model3.FileReferences.Motions[ACTION_GROUP] = actions.map((action) => ({
    File: `${MOTION_DIR}/${action.fileName}`,
    Name: ACTION_DEFS[action.name].label,
    FadeInTime: ACTION_DEFS[action.name].fadeIn,
    FadeOutTime: ACTION_DEFS[action.name].fadeOut,
  }))
  writes.push({ path: analysis.model3Path, content: serialize(model3) })

  return writes
}

export function buildCatalog(results: ModelResult[]): string {
  const entries = results
    .filter((result) => result.actions.length > 0)
    .map((result) => {
      const actions = result.actions
        .map((action, index) => {
          const def = ACTION_DEFS[action.name]
          const params = action.params.map((id) => `'${id}'`).join(', ')
          return (
            `    ${action.name}: { index: ${index}, duration: ${def.duration}, ` +
            `fadeIn: ${def.fadeIn}, fadeOut: ${def.fadeOut}, params: [${params}] },`
          )
        })
        .join('\n')
      return `  ${JSON.stringify(result.analysis.stem)}: {\n${actions}\n  },`
    })
    .join('\n')

  return `// GERADO POR scripts/live2d-gen-motions.mts — NÃO EDITE À MÃO.
// Regenerar: pnpm live2d:gen · Conferir: pnpm check:live2d-motions
/**
 * Ações disponíveis por modelo, com o índice delas no grupo \`Action\` do \`model3.json\`.
 *
 * \`params\` é a máscara: enquanto a ação toca, o animador procedural NÃO escreve nesses parâmetros.
 * Sem a máscara a motion é calculada e sobrescrita no mesmo frame, e o efeito visível é zero.
 */
export interface Live2DActionEntry {
  /** Posição no grupo \`Action\` — é por índice que o pixi-live2d-display toca. */
  readonly index: number
  readonly duration: number
  readonly fadeIn: number
  readonly fadeOut: number
  /** Parâmetros que esta motion dirige. O animador cede estes canais enquanto ela toca. */
  readonly params: readonly string[]
}

/** Chave externa: o nome do arquivo \`.model3.json\` sem extensão (\`Mao\`, \`haru_greeter_t05\`, ...). */
export const LIVE2D_ACTION_CATALOG: Readonly<Record<string, Readonly<Record<string, Live2DActionEntry>>>> = {
${entries}
}

export const LIVE2D_ACTION_GROUP = '${ACTION_GROUP}'
`
}
