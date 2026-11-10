import type { VTuberExpression } from '@mika/sdk/contracts'
import { useCallback } from 'react'
import type { getElectronApi } from '../lib/shell-api'

type ElectronApi = ReturnType<typeof getElectronApi>

type Params = {
  eAPI: ElectronApi
  input: string
  setInput: (v: string) => void
  showInput: boolean
  closeOverlays: () => void
  stopTTS: () => void
  sendMessage: (text: string) => Promise<void>
  setPinned: (v: boolean) => void
  setMicEnabled: (updater: boolean | ((prev: boolean) => boolean)) => void
  setExpression: (expr: VTuberExpression, intensity?: number) => void
  spawnReaction: (x: number, y: number) => void
  scheduleExpressionReset: (fn: () => void, ms: number) => void
  mousePosRef: { current: { x: number; y: number } | undefined }
}

/** Small UI-triggered callbacks: sending the composer, pin toggle, avatar interaction, mouse tracking, overlay close. */
export function useShellUiActions({
  eAPI,
  input,
  setInput,
  showInput,
  closeOverlays,
  stopTTS,
  sendMessage,
  setPinned,
  setMicEnabled,
  setExpression,
  spawnReaction,
  scheduleExpressionReset,
  mousePosRef,
}: Params) {
  const closeOverlaysWrapper = useCallback(() => {
    setMicEnabled((prev) => (prev && showInput ? false : prev))
    closeOverlays()
  }, [closeOverlays, showInput, setMicEnabled])

  const handleSend = useCallback(async () => {
    if (!input.trim()) return
    const msg = input.trim()
    setInput('')
    closeOverlays()
    stopTTS()
    await sendMessage(msg)
  }, [input, setInput, closeOverlays, stopTTS, sendMessage])

  const handleTogglePin = useCallback(async () => {
    const next = await eAPI.shell.window?.toggleAlwaysOnTop()
    if (next !== undefined) setPinned(next)
  }, [eAPI.shell.window, setPinned])

  const handleAvatarInteract = useCallback(
    (x?: number, y?: number) => {
      setExpression('excited')
      if (x !== undefined && y !== undefined) spawnReaction(x, y)
      scheduleExpressionReset(() => setExpression('neutral'), 2000)
    },
    [spawnReaction, setExpression, scheduleExpressionReset],
  )

  const handleMouseTrack = useCallback(
    (nx: number, ny: number) => {
      // Both avatars expect -1..1; converting to 0..1 stuck the gaze in the lower-right quadrant.
      mousePosRef.current = { x: nx, y: ny }
    },
    [mousePosRef],
  )

  return { closeOverlaysWrapper, handleSend, handleTogglePin, handleAvatarInteract, handleMouseTrack }
}
