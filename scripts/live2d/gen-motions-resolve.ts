import { ACTION_DEFS, type ActionCurve, type ActionName, CONVENTIONAL_BASES, CONVENTIONAL_RANGES } from './action-defs'
import { type BuiltCurve, buildCurve, buildMotion, clamp, type MotionFile } from './gen-motions-curve'
import type { ModelAnalysis } from './model-analysis'

export function availableParams(analysis: ModelAnalysis): Set<string> {
  if (analysis.declaredParams) return analysis.declaredParams
  return new Set(analysis.stats.keys())
}

export function safeRange(analysis: ModelAnalysis, paramId: string): { range: [number, number]; observed: boolean } {
  const stat = analysis.stats.get(paramId)
  if (stat && stat.min < stat.max) return { range: [stat.min, stat.max], observed: true }
  const conventional = CONVENTIONAL_RANGES[paramId]
  if (conventional) return { range: [conventional[0], conventional[1]], observed: false }
  return { range: [Number.NEGATIVE_INFINITY, Number.POSITIVE_INFINITY], observed: false }
}

export function resolveBase(analysis: ModelAnalysis, paramId: string): number {
  const stat = analysis.stats.get(paramId)
  if (stat) return stat.base
  return CONVENTIONAL_BASES[paramId] ?? 0
}

export function resolveParam(curve: ActionCurve, available: Set<string>): string | null {
  return curve.param.find((id) => available.has(id)) ?? null
}

export interface GeneratedAction {
  name: ActionName
  fileName: string
  motion: MotionFile
  params: string[]
}

type CurveOutcome =
  | { kind: 'missing'; paramId: string }
  | { kind: 'physics'; paramId: string }
  | { kind: 'unobserved'; error: string }
  | { kind: 'flat'; error: string }
  | { kind: 'ok'; curve: BuiltCurve; paramId: string; clamped: boolean; note?: string }

function scaleOffset(curveDef: ActionCurve, raw: number, sign: number, upSpan: number, downSpan: number): number {
  if (curveDef.unit !== 'normalized') return raw
  const offset = raw * sign
  return offset * (offset >= 0 ? upSpan : downSpan)
}

function buildCurveKeys(
  curveDef: ActionCurve,
  base: number,
  range: [number, number],
  sign: number,
  upSpan: number,
  downSpan: number,
) {
  let clamped = false
  const keys = curveDef.keys.map(([time, rawOffset]) => {
    const offset = scaleOffset(curveDef, rawOffset, sign, upSpan, downSpan)
    const value = clamp(base + offset, range[0], range[1])
    if (offset !== 0 && Math.abs(value - (base + offset)) > 1e-6) clamped = true
    return [time, value] as [number, number]
  })
  return { keys, clamped }
}

function resolveCurveOutcome(
  analysis: ModelAnalysis,
  actionName: ActionName,
  available: Set<string>,
  allowedUnobserved: Set<string>,
  curveDef: ActionCurve,
): CurveOutcome {
  const paramId = resolveParam(curveDef, available)
  if (!paramId) return { kind: 'missing', paramId: curveDef.param[0] }
  if (analysis.physicsOutputs.has(paramId)) return { kind: 'physics', paramId }

  const { range, observed } = safeRange(analysis, paramId)
  if (!observed && !allowedUnobserved.has(paramId)) {
    return {
      kind: 'unobserved',
      error:
        `'${actionName}' usa ${paramId}, que nenhuma motion original deste modelo usa — não há faixa medida.` +
        ` Decida e registre em UNOBSERVED_ALLOWLIST['${analysis.name}'] com o motivo, ou tire o canal.`,
    }
  }

  const base = resolveBase(analysis, paramId)
  const upSpan = range[1] - base
  const downSpan = base - range[0]

  const peak = curveDef.keys.reduce((acc, [, offset]) => (Math.abs(offset) > Math.abs(acc) ? offset : acc), 0)
  const wantedSpan = peak >= 0 ? upSpan : downSpan
  const otherSpan = peak >= 0 ? downSpan : upSpan
  const mirrored = curveDef.unit === 'normalized' && wantedSpan <= 1e-6 && otherSpan > 1e-6
  const sign = mirrored ? -1 : 1
  const note = mirrored
    ? `'${actionName}': ${paramId} espelhado — o rig só gira para um lado (confira na tela)`
    : undefined

  const { keys, clamped } = buildCurveKeys(curveDef, base, range, sign, upSpan, downSpan)

  if (curveDef.unit === 'normalized') {
    const values = keys.map(([, value]) => value)
    const swing = Math.max(...values) - Math.min(...values)
    if (swing < 1e-3) {
      return {
        kind: 'flat',
        error:
          `'${actionName}' usa ${paramId} normalizado e a curva saiu sem amplitude neste modelo` +
          ` (faixa ${range[0]}..${range[1]}, repouso ${base}). Escolha outro canal ou pule a ação aqui.`,
      }
    }
  }

  return { kind: 'ok', curve: buildCurve(paramId, keys), paramId, clamped, note }
}

function collectActionCurves(
  analysis: ModelAnalysis,
  name: ActionName,
  def: (typeof ACTION_DEFS)[ActionName],
  available: Set<string>,
  allowedUnobserved: Set<string>,
  physicsSkipped: Set<string>,
  notes: string[],
  errors: string[],
) {
  const curves: BuiltCurve[] = []
  const params: string[] = []
  const missing: string[] = []
  const clamped: string[] = []

  for (const curveDef of def.curves as readonly ActionCurve[]) {
    const outcome = resolveCurveOutcome(analysis, name, available, allowedUnobserved, curveDef)
    if (outcome.kind === 'missing') missing.push(outcome.paramId)
    else if (outcome.kind === 'physics') physicsSkipped.add(outcome.paramId)
    else if (outcome.kind === 'unobserved' || outcome.kind === 'flat') errors.push(outcome.error)
    else {
      if (outcome.note) notes.push(outcome.note)
      if (outcome.clamped) clamped.push(outcome.paramId)
      curves.push(outcome.curve)
      params.push(outcome.paramId)
    }
  }

  return { curves, params, missing, clamped }
}

export function buildAction(
  analysis: ModelAnalysis,
  name: ActionName,
  available: Set<string>,
  allowedUnobserved: Set<string>,
  physicsSkipped: Set<string>,
  notes: string[],
  errors: string[],
  filePrefix: string,
): GeneratedAction | null {
  const def = ACTION_DEFS[name]
  const unmet = def.requiresAnyOf.filter((group) => !group.some((id) => available.has(id)))
  if (unmet.length > 0) {
    notes.push(`pulada '${name}': o modelo não tem nenhum de ${unmet.map((g) => g.join('|')).join(' e ')}`)
    return null
  }

  const { curves, params, missing, clamped } = collectActionCurves(
    analysis,
    name,
    def,
    available,
    allowedUnobserved,
    physicsSkipped,
    notes,
    errors,
  )

  if (curves.length === 0) {
    errors.push(`'${name}' ficou sem nenhuma curva neste modelo — requiresAnyOf está frouxo demais.`)
    return null
  }

  if (missing.length > 0) notes.push(`'${name}': canais ausentes no modelo — ${[...new Set(missing)].join(', ')}`)
  if (clamped.length > 0) {
    notes.push(`'${name}': valores reduzidos à faixa observada — ${[...new Set(clamped)].join(', ')}`)
  }

  return { name, fileName: `${filePrefix}${name}.motion3.json`, motion: buildMotion(def.duration, curves), params }
}
