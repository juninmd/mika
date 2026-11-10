import { useEffect, useRef } from 'react'
import type { AppStatus } from '../components/app/app-shell.types'
import { rendererLog } from '../lib/renderer-logger'
import { nextStatusOnOfflineChange, nextStatusOnTtsStop, nextStatusOnVad } from './shell-status-transitions'

type Params = {
  appStatus: AppStatus
  setAppStatus: (status: AppStatus | ((prev: AppStatus) => AppStatus)) => void
  ttsIsPlaying: boolean
  offlineMode: boolean
  vadIsSpeaking: boolean
  micEnabled: boolean
}

/** Wires the `appStatus` transition effects (tts/offline/vad) and the debug log. The bubble fades on its own (`hud/bubble-timing.ts`). */
export function useShellStatusSync({
  appStatus,
  setAppStatus,
  ttsIsPlaying,
  offlineMode,
  vadIsSpeaking,
  micEnabled,
}: Params) {
  useEffect(() => {
    setAppStatus((s) => nextStatusOnTtsStop(ttsIsPlaying, s) ?? s)
  }, [ttsIsPlaying, setAppStatus])

  useEffect(() => {
    setAppStatus((s) => nextStatusOnOfflineChange(offlineMode, s) ?? s)
  }, [offlineMode, setAppStatus])

  useEffect(() => {
    const next = nextStatusOnVad(vadIsSpeaking, micEnabled, appStatus)
    if (next) setAppStatus(next)
  }, [vadIsSpeaking, micEnabled, appStatus, setAppStatus])

  // Logged because appStatus is invisible otherwise, making "she stopped responding" pure guesswork.
  const lastLoggedStatusRef = useRef<string | null>(null)
  useEffect(() => {
    if (lastLoggedStatusRef.current === appStatus) return
    rendererLog.debug('[SHELL] appStatus', { de: lastLoggedStatusRef.current, para: appStatus })
    lastLoggedStatusRef.current = appStatus
  }, [appStatus])
}
