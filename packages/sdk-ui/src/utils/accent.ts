import type { CSSProperties } from 'react'

export function accentVars(
  hue: number | undefined,
  options?: { saturation?: number; lightness?: number },
): CSSProperties | undefined {
  if (hue === undefined) return undefined
  const s = `${options?.saturation ?? 90}%`
  const l = `${options?.lightness ?? 65}%`
  return {
    '--accent-h': String(hue),
    '--accent-s': s,
    '--accent-l': l,
    '--accent': `hsl(${hue} ${s} ${l})`,
    '--accent-text': `hsl(${hue} ${s} 78%)`,
    '--accent-soft': `hsl(${hue} ${s} ${l} / 0.14)`,
    '--accent-rim': `hsl(${hue} ${s} ${l} / 0.32)`,
    '--accent-glow': `hsl(${hue} ${s} ${l} / 0.2)`,
  } as CSSProperties
}
