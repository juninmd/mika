const SENSITIVE_NAME_PATTERN = /key|secret|token|password|credential/i

/** Heurística por NOME: o contrato de tool de plugin não declara campo sensível, então decide pelo nome. */
export function isSensitiveArgName(name: string): boolean {
  return SENSITIVE_NAME_PATTERN.test(name)
}

/** Mascara chaves sensíveis num objeto raso; valores não-objeto voltam inalterados. */
export function maskSensitiveArgs(value: unknown): unknown {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return value
  const masked: Record<string, unknown> = {}
  for (const [key, val] of Object.entries(value as Record<string, unknown>)) {
    masked[key] = isSensitiveArgName(key) ? '***' : val
  }
  return masked
}
