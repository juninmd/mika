/**
 * check:expression-parity — pega expressão que existe no enum da tool `avatar` mas que nenhum
 * backend sabe mostrar. Já aconteceu: 19 poses entraram em `VTuberExpression` e no enum da tool sem
 * entrada em `EXPRESSION_ACTION_MAP` — a tool respondia sucesso e o avatar não movia um pixel.
 * Invisível em typecheck e em teste (os dois backends exigem WebGL e não sobem fora do navegador).
 */
import { read, report, type Violation } from './lib.mts'

const EXPRESSION_TYPES = 'packages/sdk/src/contracts/types/avatar-types.ts'
// Stage C (2026-09-22): a tool `avatar` (e a lista `EXPRESSIONS`) saiu de TS para Rust junto com
// `src/features/character` não-renderer — `model_inventory.rs` é a nova fonte.
const AVATAR_TOOL = 'apps/desktop/src-tauri/src/modules/features/character/model_inventory.rs'
const LIVE2D_CONFIG = 'apps/desktop/src/renderer/src/components/live2d/live2d-animator-config.ts'
const LIVE2D_ACTIONS = 'scripts/live2d/action-defs.ts'
const VRM_LOOK = 'apps/desktop/src/renderer/src/components/avatar/vrm-look.ts'

const quoted = (body: string): Set<string> => new Set([...body.matchAll(/'([^']+)'/g)].map((m) => m[1]))

const add = (v: Violation[], file: string, message: string): number => v.push({ file, line: 1, message })

/** Corpo de um bloco delimitado por `pattern`, ou `''` se o bloco não existir mais. */
const block = (source: string, pattern: RegExp): string => pattern.exec(source)?.[1] ?? ''

/** `chave: ['a']` ou `'chave-kebab': ['b']` dentro de um objeto — devolve chave -> valores citados. */
function actionMap(body: string): Map<string, string[]> {
  const map = new Map<string, string[]>()
  for (const match of body.matchAll(/^\s*(?:'([^']+)'|(\w+)):\s*\[([^\]]*)\]/gm)) {
    map.set(match[1] ?? match[2], [...quoted(match[3])])
  }
  return map
}

/** Expressões da tool e da union têm de ser o mesmo conjunto — uma sem a outra é decoração. */
function expressionEnumViolations(toolExpr: Set<string>, typeExpr: Set<string>): Violation[] {
  const violations: Violation[] = []
  for (const expr of toolExpr) {
    if (!typeExpr.has(expr)) {
      add(violations, AVATAR_TOOL, `EXPRESSIONS lista "${expr}", que VTUBER_EXPRESSIONS não declara`)
    }
  }
  for (const expr of typeExpr) {
    if (!toolExpr.has(expr)) {
      add(
        violations,
        EXPRESSION_TYPES,
        `VTUBER_EXPRESSIONS declara "${expr}", que EXPRESSIONS (tool avatar) não lista — a tool recusaria a pose`,
      )
    }
  }
  return violations
}

/** Toda expressão precisa virar uma motion Live2D real, e a motion precisa existir de fato. */
function live2dViolations(
  expressions: Set<string>,
  exprMap: Map<string, string[]>,
  actionDefs: Set<string>,
): Violation[] {
  const violations: Violation[] = []
  for (const expr of expressions) {
    if (expr === 'neutral') continue
    if ((exprMap.get(expr) ?? []).length === 0) {
      add(
        violations,
        LIVE2D_CONFIG,
        `"${expr}" não tem entrada (ou está vazia) em EXPRESSION_ACTION_MAP — o Live2D descarta a pose em silêncio`,
      )
    }
  }
  for (const [expr, actions] of exprMap) {
    for (const action of actions) {
      if (!actionDefs.has(action)) {
        add(
          violations,
          LIVE2D_CONFIG,
          `EXPRESSION_ACTION_MAP["${expr}"] cita a motion "${action}", que não existe em ACTION_DEFS`,
        )
      }
    }
  }
  return violations
}

/**
 * Toda expressão precisa resolver no VRM por algum caminho: gesto de corpo (VRM_EXPRESSION_ACTIONS,
 * o próprio nome em VRM_BODY_ACTIONS, ou 'nod'), ou ao menos expressão facial em EXPRESSION_PRESET.
 * `serious`, `sleep`, `wink` e `evil` são faciais por design e nunca tiveram gesto.
 */
function vrmViolations(
  expressions: Set<string>,
  vrmMap: Map<string, string[]>,
  bodyActions: Set<string>,
  facePresets: Set<string>,
): Violation[] {
  const violations: Violation[] = []
  for (const expr of expressions) {
    if (expr === 'neutral' || expr === 'nod') continue
    if (!vrmMap.has(expr) && !bodyActions.has(expr) && !facePresets.has(expr)) {
      add(
        violations,
        VRM_LOOK,
        `"${expr}" não resolve no VRM: nem gesto (VRM_EXPRESSION_ACTIONS/VRM_BODY_ACTIONS) nem rosto (EXPRESSION_PRESET)`,
      )
    }
  }
  for (const [expr, actions] of vrmMap) {
    for (const action of actions) {
      if (action !== 'nod' && !bodyActions.has(action)) {
        add(
          violations,
          VRM_LOOK,
          `VRM_EXPRESSION_ACTIONS["${expr}"] cita a ação "${action}", que não está em VRM_BODY_ACTIONS`,
        )
      }
    }
  }
  return violations
}

const EXPRESSION_PRESET_RE = /export const EXPRESSION_PRESET: Record<string, string> = \{([\s\S]*?)\n\}/

const toolExpressions = new Set(
  [...block(read(AVATAR_TOOL), /pub const EXPRESSIONS: &\[&str\] = &\[([\s\S]*?)\];/).matchAll(/"([^"]+)"/g)].map(
    (m) => m[1],
  ),
)
const typeExpressions = quoted(
  block(read(EXPRESSION_TYPES), /export const VTUBER_EXPRESSIONS = \[([\s\S]*?)\] as const/),
)
const bodyActions = quoted(block(read(EXPRESSION_TYPES), /export const VRM_BODY_ACTIONS = \[([\s\S]*?)\] as const/))
const expressionMap = actionMap(
  block(read(LIVE2D_CONFIG), /export const EXPRESSION_ACTION_MAP: Record<string, string\[\]> = \{([\s\S]*?)\n\}/),
)
const vrmExpressionActions = actionMap(
  block(read(VRM_LOOK), /export const VRM_EXPRESSION_ACTIONS: Record<string, readonly string\[\]> = \{([\s\S]*?)\n\}/),
)
const facePresetBody = block(read(VRM_LOOK), EXPRESSION_PRESET_RE)
const facePresets = new Set([...facePresetBody.matchAll(/^\s*(\w+):/gm)].map((m) => m[1]))
const actionDefsBody = block(read(LIVE2D_ACTIONS), /export const ACTION_DEFS = \{([\s\S]*?)\nexport type ActionName/)
const actionDefs = new Set([...actionDefsBody.matchAll(/^\s{2}(\w+):\s*\{/gm)].map((m) => m[1]))

if (typeExpressions.size === 0 || toolExpressions.size === 0) {
  process.stdout.write('✗ check:expression-parity — VTUBER_EXPRESSIONS ou EXPRESSIONS não encontrado\n')
  process.exit(1)
}

const violations: Violation[] = [
  ...expressionEnumViolations(toolExpressions, typeExpressions),
  ...live2dViolations(typeExpressions, expressionMap, actionDefs),
  ...vrmViolations(typeExpressions, vrmExpressionActions, bodyActions, facePresets),
]

report('check:expression-parity', violations, typeExpressions.size * 2)
