/** Lê `a.b.c` num objeto de config; uma chave literal com ponto ganha do caminho aninhado. */
export function getByPath(obj: Record<string, unknown>, path: string): unknown {
  if (obj && Object.hasOwn(obj, path)) return obj[path]
  let current: unknown = obj
  for (const key of path.split('.')) {
    if (!current || typeof current !== 'object') return undefined
    if (!Object.hasOwn(current, key)) return undefined
    current = (current as Record<string, unknown>)[key]
  }
  return current
}
