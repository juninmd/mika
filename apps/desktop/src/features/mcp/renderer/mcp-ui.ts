import { MOTION_CLASS } from '@mika/sdk-ui/settings'

const FOCUS = 'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus'
const BASE = `inline-flex items-center justify-center gap-1.5 font-bold disabled:cursor-not-allowed disabled:opacity-50 ${MOTION_CLASS.interactive} ${MOTION_CLASS.press} ${FOCUS}`

/** Botões da tela de MCP — tokens semânticos; o CTA brilha com o `--accent-glow` do tema. */
export const BTN = {
  primary: `${BASE} rounded-xl border border-accent-rim bg-accent-soft px-4 py-2 text-xs text-text-main shadow-[0_0_18px_var(--accent-glow)] hover:bg-accent/25`,
  hero: `${BASE} rounded-xl border border-accent-rim bg-accent-soft px-5 py-3 text-sm text-text-main shadow-[0_0_24px_var(--accent-glow)] hover:bg-accent/25`,
  ghost: `${BASE} rounded-lg border border-border-subtle bg-surface-raise/60 px-3 py-1.5 text-caption text-text-main hover:border-accent-rim hover:bg-accent-soft hover:text-accent`,
  icon: `${BASE} h-8 w-8 rounded-lg border border-border-subtle bg-surface-raise/60 text-text-muted hover:border-accent-rim hover:bg-accent-soft hover:text-accent`,
  subtle: `${BASE} rounded-lg px-2 py-1 text-caption text-text-muted hover:bg-surface-raise hover:text-text-main`,
} as const

export const MONO_CHIP =
  'rounded-md border border-border-subtle bg-bg-deep/60 px-2 py-1 font-mono text-caption text-text-main'
