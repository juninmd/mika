import { wizardReactStepSchema } from '@mika/sdk/contracts'
import { describe, expect, it, vi } from 'vitest'

vi.mock('../../lib/shell-api', () => ({ getElectronApi: () => ({ runtime: {} }) }))

const { reactionFor } = await import('./use-mika-reaction')
const { WIZARD_STEPS } = await import('../../wizard-app-saves')

describe('reactionFor', () => {
  // Every line she says comes from the LLM: without a brain the wizard must not ask for one.
  it('never asks for a reaction before the LLM is configured', () => {
    for (const step of WIZARD_STEPS) expect(reactionFor(step, false)).toBeNull()
  })

  it('maps the eyes-and-ears step to the vision sense', () => {
    expect(reactionFor('privacy', true)).toBe('vision')
  })

  it('has no sense for language or the final screen', () => {
    expect(reactionFor('language', true)).toBeNull()
    expect(reactionFor('done', true)).toBeNull()
  })

  // A sense the IPC schema rejects would fail the call in the backend and she would stay silent.
  it('only produces senses the IPC contract accepts', () => {
    for (const step of WIZARD_STEPS) {
      const sense = reactionFor(step, true)
      if (sense) expect(wizardReactStepSchema.safeParse(sense).success).toBe(true)
    }
  })
})
