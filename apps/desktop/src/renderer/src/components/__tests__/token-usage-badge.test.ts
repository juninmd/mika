import { describe, expect, it } from 'vitest'
import { DEFAULT_SESSION_USAGE, type SessionUsage } from '../app/app-shell.types'
import { usageBadgeState } from '../token-usage-badge'

const usage = (patch: Partial<SessionUsage>): SessionUsage => ({ ...DEFAULT_SESSION_USAGE, turns: 1, ...patch })

describe('usageBadgeState', () => {
  it('hides the badge for a free local session with room left (it used to show "105 tok · ctx 42")', () => {
    expect(usageBadgeState(usage({ totalTokens: 105, lastPromptTokens: 42, contextWindow: 8192 }))).toBeNull()
  })

  it('hides before the first turn', () => {
    expect(usageBadgeState({ ...DEFAULT_SESSION_USAGE, costUsd: 1 })).toBeNull()
  })

  it('shows what a paid provider has cost so far', () => {
    expect(usageBadgeState(usage({ costUsd: 0.0123 }))).toEqual({ cost: '$0.012', contextPercent: null })
  })

  it('marks a partial cost when some turns used an unpriced model', () => {
    expect(usageBadgeState(usage({ unknownCostTurns: 2 }))?.cost).toBe('~$0.00000')
  })

  it('warns when the last prompt filled 80% or more of the context window', () => {
    expect(usageBadgeState(usage({ lastPromptTokens: 3300, contextWindow: 4096 }))).toEqual({
      cost: null,
      contextPercent: 81,
    })
  })

  it('never claims a context fill without a known window', () => {
    expect(usageBadgeState(usage({ lastPromptTokens: 99_999, contextWindow: 0 }))).toBeNull()
  })
})
