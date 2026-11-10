// No Node built-ins: the renderer imports it by subpath (`@mika/sdk/utils/dedupe`).
export function dedupeBy<T>(items: readonly T[], keyFn: (item: T) => string | number): T[] {
  const seen = new Set<string | number>()
  const result: T[] = []
  for (const item of items) {
    const key = keyFn(item)
    if (seen.has(key)) continue
    seen.add(key)
    result.push(item)
  }
  return result
}
