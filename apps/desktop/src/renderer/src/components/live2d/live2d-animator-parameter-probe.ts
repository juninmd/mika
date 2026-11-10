import type { Live2DModelInstance } from './live2d-loader'

const OPTIONAL_PARAMS = [
  'ParamBodyAngleY',
  'ParamChest',
  'ParamEyeBallForm',
  'ParamTear',
  'ParamHandChangeR',
  'ParamHandChangeL',
] as const

export function probeModelParameters(model: Live2DModelInstance): Set<string> {
  const cm = model.internalModel.coreModel as unknown as { getParameterValueById: (id: string) => number }
  const supportedParams = new Set<string>()
  for (const param of OPTIONAL_PARAMS) {
    try {
      cm.getParameterValueById(param)
      supportedParams.add(param)
    } catch {}
  }
  return supportedParams
}

export function hasModelParam(supportedParams: Set<string> | null, name: string): boolean {
  return supportedParams?.has(name) ?? false
}

export function resolveLipSyncParams(model: Live2DModelInstance): string[] {
  const internal = model.internalModel as unknown as {
    lipSyncIds?: unknown[]
    settings?: { getLipSyncParameters?: () => unknown[] }
  }

  const ids = internal?.lipSyncIds
  if (Array.isArray(ids) && ids.length > 0) return ids.filter((id: unknown): id is string => typeof id === 'string')

  const declared = internal?.settings?.getLipSyncParameters?.()
  if (Array.isArray(declared) && declared.length > 0) {
    return declared.filter((id: unknown): id is string => typeof id === 'string')
  }

  return []
}

export interface ParameterRange {
  readonly id: string
  readonly min: number
  readonly max: number
  readonly rest: number
}

export function readParameterRanges(model: Live2DModelInstance): Map<string, ParameterRange> {
  const cm = model.internalModel.coreModel as unknown as {
    _model?: {
      parameters?: {
        ids?: unknown[]
        minimumValues?: ArrayLike<number>
        maximumValues?: ArrayLike<number>
        defaultValues?: ArrayLike<number>
      }
    }
    getParameterMinimumValue?: (index: number) => number
    getParameterMaximumValue?: (index: number) => number
    getParameterDefaultValue?: (index: number) => number
  }

  const out = new Map<string, ParameterRange>()
  const ids = listModelParameters(model)
  const bulk = cm?._model?.parameters

  for (let index = 0; index < ids.length; index++) {
    const id = ids[index]
    const min = bulk?.minimumValues?.[index] ?? cm?.getParameterMinimumValue?.(index)
    const max = bulk?.maximumValues?.[index] ?? cm?.getParameterMaximumValue?.(index)
    const rest = bulk?.defaultValues?.[index] ?? cm?.getParameterDefaultValue?.(index) ?? 0

    if (!Number.isFinite(min) || !Number.isFinite(max)) continue
    out.set(id, { id, min: min as number, max: max as number, rest: Number.isFinite(rest) ? rest : 0 })
  }

  return out
}

export function listModelParameters(model: Live2DModelInstance): string[] {
  const cm = model.internalModel.coreModel as unknown as {
    _model?: { parameters?: { ids?: unknown[] } }
    getParameterCount?: () => number
    getParameterId?: (index: number) => string | { getString?: () => { s?: string } }
  }

  const ids = cm?._model?.parameters?.ids
  if (Array.isArray(ids)) return ids.filter((id: unknown): id is string => typeof id === 'string')

  if (typeof cm?.getParameterCount === 'function' && typeof cm?.getParameterId === 'function') {
    const total = cm.getParameterCount()
    const out: string[] = []
    for (let index = 0; index < total; index++) {
      const id = cm.getParameterId(index)
      out.push(typeof id === 'string' ? id : (id?.getString?.().s ?? String(id)))
    }
    return out
  }

  return []
}
