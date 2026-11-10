import { useCallback, useEffect, useRef, useState } from 'react'
import { createMicStateGuard } from '../lib/mic-state-guard'
import type { getElectronApi } from '../lib/shell-api'

type ElectronApi = ReturnType<typeof getElectronApi>

type Params = {
  eAPI: ElectronApi
  voiceConfigLoaded: boolean
  sttEnabled: boolean
  inputMode: 'ptt' | 'continuous'
}

/** Owns `micEnabled` and its IPC/OS side-effects: echo guard, main-process sync, and sleep prevention. */
export function useShellMic({ eAPI, voiceConfigLoaded, sttEnabled, inputMode }: Params) {
  const [micEnabled, setMicEnabled] = useState(false)
  // Guards against echo ping-pong (~15 Hz loop incident); loose limit allows legit rapid toggling.
  const micStateGuard = useRef(createMicStateGuard({ limit: 5, windowMs: 1000 }))

  useEffect(() => {
    if (!voiceConfigLoaded) return
    setMicEnabled(sttEnabled && inputMode === 'continuous')
  }, [voiceConfigLoaded, sttEnabled, inputMode])

  // A send cut by the burst limit is retried with the value current by then, so the button and the
  // backend never stay in opposite states (smoke 2026-09-27: button OFF while STT kept listening).
  useEffect(() => {
    const guard = micStateGuard.current
    let retry: ReturnType<typeof setTimeout> | undefined
    const send = (): void => {
      if (guard.shouldSend(micEnabled, Date.now())) eAPI.runtime.sendMicState?.(micEnabled)
      else retry = setTimeout(send, guard.retryAfterMs(Date.now()))
    }
    send()
    return () => clearTimeout(retry)
  }, [eAPI.runtime, micEnabled])

  useEffect(() => {
    if (!micEnabled) return
    void eAPI.shell.system?.preventSleep?.(true)
    return () => {
      void eAPI.shell.system?.preventSleep?.(false)
    }
  }, [eAPI.shell.system, micEnabled])

  useEffect(() => {
    return eAPI.runtime.onListeningCommand?.(({ listening }) => {
      if (!micStateGuard.current.shouldApplyEcho(listening, Date.now())) return
      setMicEnabled(listening)
    })
  }, [eAPI.runtime])

  const handleToggleMic = useCallback(() => {
    setMicEnabled((prev) => !prev)
  }, [])

  const handleSetMic = useCallback((enabled: boolean) => {
    setMicEnabled(enabled)
  }, [])

  return { micEnabled, setMicEnabled, handleToggleMic, handleSetMic }
}
