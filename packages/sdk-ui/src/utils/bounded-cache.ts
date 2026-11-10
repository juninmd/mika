export function setBounded<K, V>(cache: Map<K, V>, key: K, value: V, max: number): void {
  cache.set(key, value)
  if (cache.size <= max) return
  for (const oldest of cache.keys()) {
    cache.delete(oldest)
    if (cache.size <= max) break
  }
}
