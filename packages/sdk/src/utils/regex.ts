/** Escapes regex metacharacters so `value` matches literally inside `new RegExp(...)`. */
export function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
