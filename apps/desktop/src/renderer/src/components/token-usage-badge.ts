import type { SessionUsage } from './app/app-shell.types'

// Below this fill the context number is noise on top of the avatar; the full usage lives in Settings › LLM.
export const CONTEXT_WARN_RATIO = 0.8

export type UsageBadgeState = { cost: string | null; contextPercent: number | null }

export function formatCost(costUsd: number, hasUnknownCost: boolean): string | null {
  if (costUsd <= 0 && !hasUnknownCost) return null
  const prefix = hasUnknownCost ? '~' : ''
  const value = costUsd < 0.01 ? costUsd.toFixed(5) : costUsd.toFixed(3)
  return `${prefix}$${value}`
}

/** The overlay badge only shows what the user can act on: money spent, or a context about to overflow. */
export function usageBadgeState(usage: SessionUsage): UsageBadgeState | null {
  if (usage.turns === 0) return null
  const cost = formatCost(usage.costUsd, usage.unknownCostTurns > 0)
  const ratio = usage.contextWindow > 0 ? usage.lastPromptTokens / usage.contextWindow : 0
  const contextPercent = ratio >= CONTEXT_WARN_RATIO ? Math.min(100, Math.round(ratio * 100)) : null
  if (!cost && contextPercent === null) return null
  return { cost, contextPercent }
}
