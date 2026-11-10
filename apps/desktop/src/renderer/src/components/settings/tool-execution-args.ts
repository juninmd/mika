import type { ToolParameterDetail } from '@mika/sdk/contracts'
import type { TranslationDict } from '@mika/sdk/i18n'

export type ExecutionStatus = 'idle' | 'executing' | 'success' | 'error'

export type ArgMap = Record<string, string>

export function inferInputType(type: string): string {
  switch (type) {
    case 'number':
    case 'integer':
      return 'number'
    case 'boolean':
      return 'checkbox'
    case 'array':
    case 'object':
      return 'textarea'
    default:
      return 'text'
  }
}

function parseNonBooleanArg(
  param: ToolParameterDetail,
  raw: string,
  t: (key: keyof TranslationDict) => string,
): { value: unknown; error: string | null } {
  switch (param.type) {
    case 'number':
    case 'integer': {
      const n = Number(raw)
      return { value: Number.isNaN(n) ? raw : n, error: null }
    }
    case 'array':
    case 'object':
      try {
        return { value: JSON.parse(raw), error: null }
      } catch {
        return { value: undefined, error: `"${param.name}" ${t('settings.toolExecution.invalidJsonSuffix')}` }
      }
    default:
      return { value: raw, error: null }
  }
}

export function parseArgsBySchema(
  args: ArgMap,
  params: ToolParameterDetail[],
  t: (key: keyof TranslationDict) => string,
): { parsed: Record<string, unknown>; error: string | null } {
  const parsed: Record<string, unknown> = {}
  for (const param of params) {
    const raw = args[param.name] ?? ''
    // Booleano vazio (checkbox nunca marcado) é um "false" válido, não "campo obrigatório faltando".
    if (param.type === 'boolean') {
      parsed[param.name] = raw === 'true'
      continue
    }
    if (!raw && param.required) {
      return { parsed, error: `"${param.name}" ${t('settings.toolExecution.requiredSuffix')}` }
    }
    if (!raw) continue
    const { value, error } = parseNonBooleanArg(param, raw, t)
    if (error) return { parsed, error }
    parsed[param.name] = value
  }
  return { parsed, error: null }
}

export function argBadge(param: ToolParameterDetail, t: (key: keyof TranslationDict) => string): string {
  return param.required ? `${param.type} · ${t('settings.toolExecution.required')}` : param.type
}
