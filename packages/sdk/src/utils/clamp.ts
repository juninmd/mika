// Non-finite input returns `fallback` instead of propagating NaN into state.
export function clamp(value: number, min: number, max: number, fallback = min): number {
  return Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : fallback
}

export function clampPercent(value: number, fallback = 0): number {
  return clamp(value, 0, 100, fallback)
}

export function clampAffinity(value: number, fallback = 0): number {
  return clamp(value, -100, 100, fallback)
}

export function clampUnit(value: number, fallback = 0): number {
  return clamp(value, -1, 1, fallback)
}

export function clamp01(value: number, fallback = 0): number {
  return clamp(value, 0, 1, fallback)
}

// Not rounded: the call site rounds when the display needs a whole number.
export function bipolarToPercent(value: number, fallback = 50): number {
  if (!Number.isFinite(value)) return fallback
  return ((clampUnit(value) + 1) / 2) * 100
}
