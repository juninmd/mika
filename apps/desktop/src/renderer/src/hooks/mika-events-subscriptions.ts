import { t } from '@mika/sdk/i18n'
import type { ActiveToolState, AppStatus } from '../components/app/app-shell.types'
import { parseMatchEffectToolResult } from '../lib/match-effect'
import { rendererLog } from '../lib/renderer-logger'
import type { getElectronApi } from '../lib/shell-api'
import { isForegroundToolEvent } from '../lib/tool-call-origin'
import { getFriendlyToolLabel } from '../lib/tool-labels'
import { useChatStore } from '../store/useChatStore'
import { useEffectsStore } from '../store/useEffectsStore'
import type { useUIStore } from '../store/useUIStore'
import { type UseMikaEventsProps, updateActiveTool } from './mika-events-helpers'

type ElectronApi = ReturnType<typeof getElectronApi>
type SetAppStatus = (status: AppStatus | ((prev: AppStatus) => AppStatus)) => void
type Notify = ReturnType<typeof useUIStore.getState>['addNotification']

export function subscribeTTSAudio(
  eAPI: ElectronApi,
  setAppStatus: SetAppStatus,
  enqueueTTSRef: { current: UseMikaEventsProps['enqueueTTS'] },
  onTTSErrorRef: { current: UseMikaEventsProps['onTTSError'] },
) {
  return eAPI.runtime.onTTSAudio(({ audio, mimeType, words }) => {
    const sanitizedAudio = typeof audio === 'string' ? audio.trim() : ''
    if (!sanitizedAudio) {
      rendererLog.warn('[TTS] Ignoring empty audio payload from desktop event bridge')
      onTTSErrorRef.current?.(t('shell.error.ttsEmptyAudio'))
      return
    }
    rendererLog.info('[TTS] Audio payload received', {
      length: sanitizedAudio.length,
      mimeType,
      words: words?.length ?? 0,
    })
    setAppStatus('speaking')
    enqueueTTSRef.current(sanitizedAudio, mimeType ?? 'audio/mpeg', words)
  })
}

type ThinkingDeps = {
  setAppStatus: SetAppStatus
  setActiveTool: (tool: ActiveToolState) => void
  setBubbleVisible: (visible: boolean) => void
}

export function subscribeThinkingEvents(
  eAPI: ElectronApi,
  { setAppStatus, setActiveTool, setBubbleVisible }: ThinkingDeps,
) {
  const unsubThinkingStart = eAPI.runtime.onChatThinkingStart?.(({ origin }) => {
    // Proactive ('background') speech isn't a response to a question, so it doesn't light the thinking HUD.
    if (!isForegroundToolEvent(origin)) return
    useChatStore.getState().resetTurnSteps()
    setAppStatus('thinking')
    // Previous reply could still be in its read time when a new turn starts; new turn always wins.
    setBubbleVisible(false)
  })

  const unsubThinkingStop = eAPI.runtime.onChatThinkingStop?.(() => {
    // The only unconditional "turn ended" signal — emitted from chat.service.ts's finally, even for
    // tool-only turns that never fire onAssistantMessage.
    setAppStatus((s) => (s === 'thinking' || s === 'acting' ? 'idle' : s))
    setActiveTool(null)
    useChatStore.getState().resetTurnSteps()
  })

  return { unsubThinkingStart, unsubThinkingStop }
}

type ToolEventDeps = {
  setActiveTool: (tool: ActiveToolState) => void
  setAppStatus: SetAppStatus
  setToolClearTimer: (callback: () => void, ms: number) => void
  clearToolClearTimer: () => void
}

export function subscribeToolEvents(
  eAPI: ElectronApi,
  { setActiveTool, setAppStatus, setToolClearTimer, clearToolClearTimer }: ToolEventDeps,
) {
  const unsubToolCall = eAPI.runtime.onToolCall(({ name, origin }) => {
    // Only origin: 'turn' lights the thinking HUD; background jobs (mood/fact/context) aren't visible conversation.
    if (!isForegroundToolEvent(origin)) return
    clearToolClearTimer()
    useChatStore.getState().beginStep(name, getFriendlyToolLabel(name))
    setActiveTool({ name, startedAt: Date.now() })
    setAppStatus('acting')
  })

  const unsubToolResult = eAPI.runtime.onToolResult(({ name, durationMs, success, result, origin }) => {
    if (!isForegroundToolEvent(origin)) return
    clearToolClearTimer()
    const finishedAt = Date.now()
    useChatStore.getState().finishStep(name, success !== false)
    updateActiveTool({ name, success, durationMs, finishedAt })
    setToolClearTimer(() => setActiveTool(null), 1500)
    setAppStatus((s) => (s === 'acting' ? 'thinking' : s))

    if (name === 'play_minigame' && success) {
      eAPI.shell.pluginViews?.open({ pluginId: 'mika.gamer-companion', viewId: 'gamer-companion.mini-games' })
    }

    if (name === 'match_effect' && success) {
      const command = parseMatchEffectToolResult(result)
      if (command) useEffectsStore.getState().applyMatchEffect(command)
    }
  })

  return { unsubToolCall, unsubToolResult }
}

export function subscribeNotices(eAPI: ElectronApi, addNotification: Notify) {
  const unsubNotification = eAPI.runtime.onNotification?.(({ message, type, source }) => {
    if (!message) return
    addNotification(type ?? 'info', message, source)
  })

  return { unsubNotification }
}
