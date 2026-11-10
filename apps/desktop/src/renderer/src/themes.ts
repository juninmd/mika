import { EXTRA_THEMES } from './themes-extra'

type ThemeScheme = 'dark' | 'light'

export type Theme = {
  name: string
  id: string

  scheme: ThemeScheme
  colors: {
    background: string
    surface: string
    primary: string
    text: string
    textMuted: string
    border: string
    accent: string
    neonAccent: string
    neonSecondary: string
    neuralPurple: string
    neuralCyan: string
    neuralMint: string
    neuralCoral: string
    glassLayer: string
  }
}

export const THEMES: Theme[] = [
  {
    name: 'Neural Glass (Padrão)',
    id: 'neural-glass',
    scheme: 'dark',
    colors: {
      background: '#030406',
      surface: 'rgba(8, 10, 16, 0.55)',
      primary: '#a855f7',
      text: '#ffffff',
      textMuted: '#94a3b8',
      border: 'rgba(168, 85, 247, 0.15)',
      accent: '#bc77ff',
      neonAccent: '#a855f7',
      neonSecondary: '#20d8ff',
      neuralPurple: '#a855f7',
      neuralCyan: '#20d8ff',
      neuralMint: '#31f6a1',
      neuralCoral: '#ff6b9d',
      glassLayer: 'rgba(8, 10, 16, 0.55)',
    },
  },
  {
    name: 'Claro',
    id: 'light',
    scheme: 'light',
    colors: {
      background: '#eef3f8',
      surface: 'rgba(255, 255, 255, 0.88)',
      primary: '#6366f1',
      text: '#0f172a',
      textMuted: '#334155',
      border: 'rgba(15, 23, 42, 0.14)',
      accent: '#4f46e5',
      neonAccent: '#6366f1',
      neonSecondary: '#8b5cf6',
      neuralPurple: '#6366f1',
      neuralCyan: '#0891b2',
      neuralMint: '#059669',
      neuralCoral: '#e11d48',
      glassLayer: 'rgba(255, 255, 255, 0.88)',
    },
  },
  {
    name: 'Sakura',
    id: 'sakura',
    scheme: 'light',
    colors: {
      background: '#fff1f2',
      surface: 'rgba(255, 255, 255, 0.92)',
      primary: '#be123c',
      text: '#4c0519',
      textMuted: '#9f1239',
      border: 'rgba(190, 18, 60, 0.18)',
      accent: '#e11d48',
      neonAccent: '#db2777',
      neonSecondary: '#e11d48',
      neuralPurple: '#db2777',
      neuralCyan: '#be123c',
      neuralMint: '#b45309',
      neuralCoral: '#e11d48',
      glassLayer: 'rgba(255, 255, 255, 0.92)',
    },
  },
  {
    name: 'Cyberpunk',
    id: 'cyberpunk',
    scheme: 'dark',
    colors: {
      background: '#050510',
      surface: 'rgba(12, 12, 35, 0.72)',
      primary: '#d946ef',
      text: '#ffffff',
      textMuted: '#a8a29e',
      border: 'rgba(6, 182, 212, 0.25)',
      accent: '#22d3ee',
      neonAccent: '#06b6d4',
      neonSecondary: '#d946ef',
      neuralPurple: '#d946ef',
      neuralCyan: '#22d3ee',
      neuralMint: '#34d399',
      neuralCoral: '#fb7185',
      glassLayer: 'rgba(12, 12, 35, 0.72)',
    },
  },
  ...EXTRA_THEMES,
]

export const DEFAULT_THEME_ID = THEMES[0].id

export function getTheme(id: string): Theme {
  return THEMES.find((t) => t.id === id) || THEMES[0]
}
