import { getTheme, type Theme } from '../themes'

function hexToRgb(hex: string): [number, number, number] | null {
  const normalized = hex.trim().replace('#', '')
  const value =
    normalized.length === 3
      ? normalized
          .split('')
          .map((char) => char + char)
          .join('')
      : normalized
  if (value.length !== 6 || !/^[0-9a-f]{6}$/i.test(value)) return null
  return [
    Number.parseInt(value.slice(0, 2), 16),
    Number.parseInt(value.slice(2, 4), 16),
    Number.parseInt(value.slice(4, 6), 16),
  ]
}

export function toAccentTriple(color: string): { h: number; s: number; l: number } | null {
  const rgb = hexToRgb(color)
  if (!rgb) return null
  const [r, g, b] = rgb.map((c) => c / 255) as [number, number, number]
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const l = (max + min) / 2
  const delta = max - min
  if (delta === 0) return { h: 0, s: 0, l: Math.round(l * 100) }
  const s = delta / (1 - Math.abs(2 * l - 1))
  let h: number
  if (max === r) h = ((g - b) / delta) % 6
  else if (max === g) h = (b - r) / delta + 2
  else h = (r - g) / delta + 4
  h = Math.round(h * 60)
  if (h < 0) h += 360
  return { h, s: Math.round(s * 100), l: Math.round(l * 100) }
}

export function applyTheme(theme: Theme): void {
  const root = document.documentElement
  const { colors } = theme
  root.dataset.theme = theme.id

  root.dataset.scheme = theme.scheme

  root.style.setProperty('--bg', colors.background)
  root.style.setProperty('--surface', colors.surface)
  root.style.setProperty('--glass-layer', colors.glassLayer)
  root.style.setProperty('--primary', colors.primary)
  root.style.setProperty('--text', colors.text)
  root.style.setProperty('--text-muted', colors.textMuted)
  root.style.setProperty('--border', colors.border)

  const accent = toAccentTriple(colors.accent)
  if (accent) {
    root.style.setProperty('--accent-h', String(accent.h))
    root.style.setProperty('--accent-s', `${accent.s}%`)
    root.style.setProperty('--accent-l', `${theme.scheme === 'light' ? Math.min(accent.l, 45) : accent.l}%`)
  }

  root.style.setProperty('--neon-primary', colors.neonAccent)
  root.style.setProperty('--neural-purple', colors.neuralPurple)
  root.style.setProperty('--neural-cyan', colors.neuralCyan)
  root.style.setProperty('--neural-mint', colors.neuralMint)
  root.style.setProperty('--neural-coral', colors.neuralCoral)
}

export function applyThemeById(id: string): void {
  applyTheme(getTheme(id))
}
