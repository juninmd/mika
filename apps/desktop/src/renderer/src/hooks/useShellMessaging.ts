import type { CrisisRegion, VTuberExpression } from '@mika/sdk/contracts'
import { t } from '@mika/sdk/i18n'
import { isNetworkError, toDisplayError } from '@mika/sdk-ui'
import { useCallback, useRef, useState } from 'react'
import type { AppMessage, AppStatus, MessageSource } from '../components/app/app-shell.types'
import { rendererLog } from '../lib/renderer-logger'
import type { getElectronApi } from '../lib/shell-api'
import { isHeldSpeechStale } from '../lib/speech-turn'
import { useChatStore } from '../store/useChatStore'
import { useEffectsStore } from '../store/useEffectsStore'
import { stripActionTags } from './mika-events-helpers'
import { type ClearGuard, handleSlashCommand } from './shell-commands'
import { appendAssistantMessage, isFailedTurn, toShortErrorMessage } from './shell-message-utils'

type ElectronApi = ReturnType<typeof getElectronApi>
type NotifyFn = (type: 'info' | 'success' | 'warning' | 'error', message: string, title?: string) => void

type Params = {
  eAPI: ElectronApi
  messages: AppMessage[]
  setMessages: (messages: AppMessage[] | ((prev: AppMessage[]) => AppMessage[])) => void
  setBubble: (text: string, visible: boolean) => void
  setBubbleVisible: (v: boolean) => void
  setAppStatus: (status: AppStatus | ((prev: AppStatus) => AppStatus)) => void
  addNotification: NotifyFn
  setExpression: (expr: VTuberExpression, intensity?: number) => void
  scheduleExpressionReset: (fn: () => void, ms: number) => void
}

/** Owns a chat turn end-to-end: dispatch, response/error handling, slash commands, and held speech. */
export function useShellMessaging({
  eAPI,
  messages,
  setMessages,
  setBubble,
  setBubbleVisible,
  setAppStatus,
  addNotification,
  setExpression,
  scheduleExpressionReset,
}: Params) {
  const pendingMessageRef = useRef<string | null>(null)
  const sendMessageSeqRef = useRef(0)
  // Sync mutex: appStatus only reflects 'thinking' after the next render, so two closures could race on 'idle'.
  const isSendingRef = useRef(false)
  const heldSpeechRef = useRef<{ text: string; at: number } | null>(null)
  const flushHeldSpeechRef = useRef<(() => void) | null>(null)
  const [speechHeld, setSpeechHeld] = useState(false)

  const handleSendResponse = useCallback(
    (response: { content?: string; id?: string; role?: string; crisisRegion?: CrisisRegion }) => {
      if (response.crisisRegion) useChatStore.getState().setCrisisRegion(response.crisisRegion)
      const responseContent = stripActionTags(response.content ?? '')
      if (responseContent) {
        setBubble(responseContent, true)
        // A failed turn is shown (bubble + error status from `chat:error`) but never filed as something she said.
        if (!isFailedTurn(response)) {
          setMessages((prev) => appendAssistantMessage(prev, { id: response.id, content: responseContent }))
        }
        setAppStatus((s) => (s === 'thinking' ? 'idle' : s))
      } else {
        setAppStatus('idle')
      }
      scheduleExpressionReset(() => setExpression('neutral'), 4000)
    },
    [setBubble, setMessages, setAppStatus, setExpression, scheduleExpressionReset],
  )

  const handleSendError = useCallback(
    (error: unknown) => {
      setAppStatus('error')
      setExpression('sad')
      const msg = toDisplayError(error, t('shell.error.sendMessage'))
      const shortMsg = toShortErrorMessage(msg)

      setBubbleVisible(false)
      addNotification('error', shortMsg, t(isNetworkError(msg) ? 'shell.error.offlineTitle' : 'shell.error.commTitle'))
      scheduleExpressionReset(() => {
        setExpression('neutral')
        setAppStatus('idle')
      }, 4000)
    },
    [setAppStatus, setBubbleVisible, addNotification, setExpression, scheduleExpressionReset],
  )

  const doClearHistory = useCallback(async () => {
    await eAPI.runtime.clearChatHistory().catch(() => rendererLog.warn('[useAppShell] failed to clear chat history'))
    setMessages([])
  }, [eAPI.runtime, setMessages])

  const clearGuardRef = useRef<ClearGuard>({ armedAt: null })
  const handleCommand = useCallback(
    (text: string): boolean =>
      handleSlashCommand({
        text,
        messages,
        clearGuard: clearGuardRef.current,
        doClearHistory,
        setBubbleVisible,
        addNotification,
        setAppStatus,
        onEffectCommand: (effect) => {
          useEffectsStore.getState().applyMatchEffect({ effect, mode: 'toggle' })
        },
      }),
    [doClearHistory, messages, addNotification, setAppStatus, setBubbleVisible],
  )

  const endTurn = useCallback(() => {
    pendingMessageRef.current = null
    isSendingRef.current = false
    queueMicrotask(() => flushHeldSpeechRef.current?.())
  }, [])

  const runTurn = useCallback(
    async (normalizedText: string, source: MessageSource, seq: number) => {
      try {
        const response = await eAPI.runtime.sendChatMessage(normalizedText, undefined, source)
        if (sendMessageSeqRef.current === seq) handleSendResponse(response)
      } catch (error) {
        if (sendMessageSeqRef.current === seq) handleSendError(error)
      } finally {
        // Turn ends only here; tying it to appStatus === 'idle' depended on a UI transition that might never come.
        endTurn()
      }
    },
    [eAPI.runtime, endTurn, handleSendResponse, handleSendError],
  )

  const sendMessage = useCallback(
    async (text: string, source: MessageSource = 'text') => {
      const normalizedText = text.trim()
      if (!normalizedText || pendingMessageRef.current === normalizedText) return
      if (isSendingRef.current) return
      isSendingRef.current = true
      pendingMessageRef.current = normalizedText

      if (normalizedText.startsWith('/') && handleCommand(normalizedText)) {
        endTurn()
        return
      }

      const seq = ++sendMessageSeqRef.current

      const userMsg: AppMessage = {
        id: crypto.randomUUID(),
        role: 'user',
        content: normalizedText,
        timestamp: Date.now(),
        source,
      }
      setMessages((prev) => [...prev, userMsg])
      setAppStatus('thinking')
      setExpression('thinking')
      setBubble('...', true)
      await runTurn(normalizedText, source, seq)
    },
    [runTurn, endTurn, handleCommand, setMessages, setAppStatus, setBubble, setExpression],
  )

  // Releases only the LAST held utterance; past HELD_SPEECH_TTL_MS it's dropped with a log instead.
  flushHeldSpeechRef.current = () => {
    const held = heldSpeechRef.current
    if (!held || isSendingRef.current) return

    heldSpeechRef.current = null
    setSpeechHeld(false)

    if (isHeldSpeechStale(held.at, Date.now())) {
      rendererLog.info('[STT] fala guardada expirou antes do turno liberar', {
        ageMs: Date.now() - held.at,
        chars: held.text.length,
      })
      return
    }
    rendererLog.info('[STT] soltando a fala guardada agora que o turno terminou', { chars: held.text.length })
    void sendMessage(held.text, 'voice')
  }

  const holdSpeech = useCallback((text: string) => {
    heldSpeechRef.current = { text, at: Date.now() }
    setSpeechHeld(true)
  }, [])

  return { sendMessage, doClearHistory, isSendingRef, speechHeld, holdSpeech }
}
