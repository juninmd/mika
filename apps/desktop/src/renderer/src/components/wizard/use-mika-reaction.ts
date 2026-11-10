import type { WizardReactStep } from '@mika/sdk/contracts'
import { useCallback, useRef, useState } from 'react'
import { stripActionTags } from '../../hooks/mika-events-helpers'
import { rendererLog } from '../../lib/renderer-logger'
import { runtimeApi } from '../../wizard-app-saves'
import type { Step } from './wizard-data'

// Which sense each finished step hands her. `language` has none: it only sets the UI locale.
const SENSE_OF_STEP: Partial<Record<Step, WizardReactStep>> = {
  avatar: 'avatar',
  llm: 'llm',
  tts: 'tts',
  embedding: 'embedding',
  privacy: 'vision',
  identity: 'identity',
}

/** Without a brain there is nobody to react: every line she says comes from the LLM. */
export function reactionFor(step: Step, llmConfigured: boolean): WizardReactStep | null {
  return llmConfigured ? (SENSE_OF_STEP[step] ?? null) : null
}

export type MikaReaction = { text: string; pending: boolean }

export function useMikaReaction() {
  const [reaction, setReaction] = useState<MikaReaction>({ text: '', pending: false })
  const latest = useRef(0)

  // Fire-and-forget: navigation never waits for the LLM, and a slower older reply never
  // overwrites a newer one.
  const react = useCallback((sense: WizardReactStep) => {
    const id = ++latest.current
    setReaction((prev) => ({ ...prev, pending: true }))
    runtimeApi()
      .wizardReact({ step: sense })
      .then(({ text }) => {
        if (id !== latest.current) return
        const line = stripActionTags(text).trim()
        setReaction((prev) => ({ text: line || prev.text, pending: false }))
      })
      .catch((error: unknown) => {
        rendererLog.warn('[Wizard] Mika reaction failed', { sense, error: String(error) })
        if (id === latest.current) setReaction((prev) => ({ ...prev, pending: false }))
      })
  }, [])

  return { reaction, react }
}
