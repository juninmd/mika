export const SENSITIVE_KEY_RE =
  /(token|secret|password|authorization|cookie|api[_-]?key|client[_-]?id|session|private[_-]?key|openrouter[_-]?key)/i
export const PRIVATE_CONTENT_KEY_RE = /(query|url|history|recentwatched|recentsearches|title)/i

const MAX_VALUE_LENGTH = 200
const MAX_REDACTION_DEPTH = 6

function isSecretCandidate(key: string, value: unknown): boolean {
  if (!SENSITIVE_KEY_RE.test(key)) return false
  return typeof value !== 'number' && typeof value !== 'boolean'
}

export function redactArgs(
  args: Record<string, unknown> | null | undefined,
  includePrivateContent = true,
): Record<string, unknown> {
  if (!args) return {}
  return redactValue(args, includePrivateContent, 0) as Record<string, unknown>
}

export function redactLogMeta(meta: Record<string, unknown> | undefined): Record<string, unknown> | undefined {
  if (!meta) return meta
  return redactValue(meta, false, 0) as Record<string, unknown>
}

function redactValue(value: unknown, includePrivateContent: boolean, depth: number): unknown {
  if (value === null || typeof value !== 'object') return value
  // Além do limite devolve marcador, nunca a subárvore crua — um `apiKey` no nível 7 sairia em texto puro.
  if (depth >= MAX_REDACTION_DEPTH) return '[TRUNCATED]'
  if (Array.isArray(value)) {
    return value.map((item) => redactValue(item, includePrivateContent, depth + 1))
  }
  const result: Record<string, unknown> = {}
  for (const [key, item] of Object.entries(value as Record<string, unknown>)) {
    if (isSecretCandidate(key, item)) {
      result[key] = '[REDACTED]'
    } else if (includePrivateContent && PRIVATE_CONTENT_KEY_RE.test(key)) {
      result[key] = '[REDACTED_CONTENT]'
    } else if (typeof item === 'string' && item.length > MAX_VALUE_LENGTH) {
      result[key] = `${item.slice(0, MAX_VALUE_LENGTH)}...`
    } else {
      result[key] = redactValue(item, includePrivateContent, depth + 1)
    }
  }
  return result
}

export function summarizeResult(result: unknown): Record<string, unknown> {
  if (typeof result !== 'object' || result === null) return { resultType: typeof result }
  const value = result as Record<string, unknown>
  return {
    success: typeof value.success === 'boolean' ? value.success : undefined,
    hasData: Object.hasOwn(value, 'data'),
    hasError: Object.hasOwn(value, 'error'),
  }
}

export function jsonFailureReason(error: unknown): string {
  if (!(error instanceof Error)) return 'erro desconhecido ao ler JSON'
  const position = error.message.match(/position (\d+)/)?.[1]
  return position ? `${error.name} na posição ${position}` : error.name
}
