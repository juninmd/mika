import { useCallback, useEffect, useRef, useState } from 'react'
import { MAX_HISTORY, type MoodRecord, type MoodState } from '../components/mood-hud-config'
import { getElectronApi } from '../lib/shell-api'
import { useTransientFlag } from './useTransientFlag'

/** Mika's live mood: initial silent read of the `mood` tool, then `mood:changed` pushes. */
export function useMoodState(): { mood: MoodState | null; history: MoodRecord[]; pulse: boolean } {
  const [mood, setMood] = useState<MoodState | null>(null)
  const [history, setHistory] = useState<MoodRecord[]>([])
  const [pulse, triggerPulse] = useTransientFlag(600)
  const moodRef = useRef<MoodState | null>(null)
  const moodSeqRef = useRef(0)

  const recordMood = useCallback((next: MoodState) => {
    const previous = moodRef.current
    moodRef.current = next
    if (previous) setHistory((h) => [...h.slice(-(MAX_HISTORY - 1)), { ...previous, at: Date.now() }])
    setMood(next)
  }, [])

  useEffect(() => {
    const eAPI = getElectronApi()
    if (!eAPI?.runtime?.onMoodChanged) return

    const fetchInitialMood = async () => {
      const seq = moodSeqRef.current
      try {
        // Leitura silenciosa do próprio estado ao montar — não é uma ferramenta de conversa, e
        // marcada 'background' não acende o ThinkingHUD sobre o avatar.
        const result = (await eAPI.runtime.executeTool('mood', { action: 'get' }, 'background')) as {
          success?: boolean
          data?: { mood?: MoodState }
        }
        if (moodSeqRef.current !== seq) return
        if (result.success && result.data?.mood) recordMood(result.data.mood)
      } catch {}
    }

    const unsub = eAPI.runtime.onMoodChanged((m) => {
      moodSeqRef.current++
      recordMood(m)
      triggerPulse()
    })
    void fetchInitialMood()
    return unsub
  }, [recordMood, triggerPulse])

  return { mood, history, pulse }
}
