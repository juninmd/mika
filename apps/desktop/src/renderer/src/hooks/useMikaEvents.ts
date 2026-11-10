import { t } from '@mika/sdk/i18n'
import { useEffect, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import { isForegroundToolEvent } from '../lib/tool-call-origin'
import { useChatStore } from '../store/useChatStore'
import { useUIStore } from '../store/useUIStore'
import {
  accumulateSessionUsage,
  aggregateChunks,
  leadingAvatarTag,
  sanitizeStreamBatch,
  stripActionTags,
  type UseMikaEventsProps,
} from './mika-events-helpers'
import {
  subscribeNotices,
  subscribeThinkingEvents,
  subscribeToolEvents,
  subscribeTTSAudio,
} from './mika-events-subscriptions'
import { useTimeoutRef } from './useTransientFlag'

export function useMikaEvents({ enqueueTTS, setExpression, onTTSError, stopTTS }: UseMikaEventsProps) {
  const { set: setToolClearTimer, clear: clearToolClearTimer } = useTimeoutRef()
  const { set: setNeutralTimer, clear: clearNeutralTimer } = useTimeoutRef()
  const isReceivingRef = useRef(false)
  const streamUiActiveRef = useRef(false)
  const onTTSErrorRef = useRef(onTTSError)
  const stopTTSRef = useRef(stopTTS)
  useEffect(() => {
    stopTTSRef.current = stopTTS
  }, [stopTTS])

  const { setAppStatus, setBubble, setBubbleVisible, setMessages, setActiveTool, setSessionUsage } = useChatStore(
    useShallow((s) => ({
      setAppStatus: s.setAppStatus,
      setBubble: s.setBubble,
      setBubbleVisible: s.setBubbleVisible,
      setMessages: s.setMessages,
      setActiveTool: s.setActiveTool,
      setSessionUsage: s.setSessionUsage,
    })),
  )
  const addNotification = useUIStore((s) => s.addNotification)

  const enqueueTTSRef = useRef(enqueueTTS)
  useEffect(() => {
    enqueueTTSRef.current = enqueueTTS
  }, [enqueueTTS])
  useEffect(() => {
    onTTSErrorRef.current = onTTSError
  }, [onTTSError])

  useEffect(() => {
    let chunkCount = 0

    const eAPI = getElectronApi()
    const aggregator = aggregateChunks()

    // Raw, untouched — a tag can split across two flushes (`<|SM` + `UG|>`), so sanitizing has to
    // re-run over the whole accumulation, never just the newest batch.
    let rawAccumulated = ''
    // The reply's opening tag (`<|WAVE|>`) is the pose she strikes; read once per turn.
    let tagRead = false
    const applyOpeningTag = (text: string) => {
      if (tagRead) return
      const tag = leadingAvatarTag(text)
      if (tag === 'pending') return
      tagRead = true
      if (!tag) return
      clearNeutralTimer()
      setExpression(tag.expression, tag.intensity)
    }
    aggregator.onFlush((batch: string) => {
      if (chunkCount > 30) {
        rendererLog.warn(`Stream render pressure: ${chunkCount} chunks since last flush`)
      }
      chunkCount = 0
      rawAccumulated += batch
      applyOpeningTag(rawAccumulated)
      if (!isReceivingRef.current) {
        isReceivingRef.current = true
      }
      useChatStore.setState({ bubbleText: sanitizeStreamBatch(rawAccumulated) })
    })

    const unsubTTS = subscribeTTSAudio(eAPI, setAppStatus, enqueueTTSRef, onTTSErrorRef)

    const unsubChunk = eAPI.runtime.onChatStreamChunk(({ text, origin }) => {
      chunkCount++
      // Proactive speech streams text like any turn but isn't "thinking" about anyone's question.
      if (!isForegroundToolEvent(origin)) return
      if (!streamUiActiveRef.current) {
        streamUiActiveRef.current = true
        setAppStatus('thinking')
        setBubbleVisible(true)
      }
      aggregator.push(text)
    })

    const { unsubThinkingStart, unsubThinkingStop } = subscribeThinkingEvents(eAPI, {
      setAppStatus,
      setActiveTool,
      setBubbleVisible,
    })

    const unsubExpr = eAPI.runtime.onAvatarExpression((data) => {
      const payload = data as typeof data & { intensity?: number }
      setExpression(payload.expression, payload.intensity)
    })

    const unsubMsg = eAPI.runtime.onAssistantMessage(({ content, timestamp, usage }) => {
      aggregator.flush()
      // Replies that never streamed (or streamed only a partial tag) still get their pose.
      if (content) applyOpeningTag(content)
      tagRead = false
      rawAccumulated = ''
      isReceivingRef.current = false
      streamUiActiveRef.current = false
      if (usage) {
        setSessionUsage((prev) => accumulateSessionUsage(prev, usage))
      }
      if (content) {
        const displayContent = stripActionTags(content)
        setMessages((prev) => {
          const last = prev[prev.length - 1]
          if (last?.role === 'assistant' && last.content === displayContent) return prev
          return [...prev, { id: crypto.randomUUID(), role: 'assistant', content: displayContent, timestamp }]
        })

        // Fade is the bubble's own job (`hud/bubble-timing.ts`): reading time counted from here,
        // and never while she is still speaking it.
        setBubble(displayContent, true)
      }
      setAppStatus((s) => (s === 'thinking' ? 'idle' : s))
      setActiveTool(null)
      useChatStore.getState().resetTurnSteps()
      setNeutralTimer(() => setExpression('neutral'), 4000)
    })

    const { unsubToolCall, unsubToolResult } = subscribeToolEvents(eAPI, {
      setActiveTool,
      setAppStatus,
      setToolClearTimer,
      clearToolClearTimer,
    })

    const unsubChatError = eAPI.runtime.onChatError?.(({ message }) => {
      aggregator.flush()
      tagRead = false
      rawAccumulated = ''
      isReceivingRef.current = false
      streamUiActiveRef.current = false
      setAppStatus('error')
      setExpression('sad')
      setActiveTool(null)
      useChatStore.getState().resetTurnSteps()
      const shortMsg = message.length > 120 ? t('shell.error.processFailed') : message

      setBubbleVisible(false)
      addNotification('error', shortMsg, t('shell.error.commTitle'))
      setNeutralTimer(() => {
        setExpression('neutral')
        setAppStatus('idle')
      }, 4000)
    })

    const unsubForceStop = eAPI.runtime.onForceStopTts?.(() => {
      stopTTSRef.current?.()
      aggregator.flush()
      tagRead = false
      rawAccumulated = ''
      isReceivingRef.current = false
      streamUiActiveRef.current = false
      setAppStatus('idle')
    })

    const { unsubNotification } = subscribeNotices(eAPI, addNotification)

    const cleanups: Array<(() => void) | undefined> = [
      unsubForceStop,
      unsubNotification,
      unsubTTS,
      unsubChunk,
      unsubExpr,
      unsubMsg,
      unsubToolCall,
      unsubToolResult,
      unsubThinkingStart,
      unsubThinkingStop,
      unsubChatError,
    ]
    return () => {
      aggregator.reset()
      for (const fn of cleanups) fn?.()
      clearNeutralTimer()
      clearToolClearTimer()
    }
  }, [
    setAppStatus,
    setBubble,
    setBubbleVisible,
    setMessages,
    setActiveTool,
    setSessionUsage,
    setExpression,
    addNotification,
    setNeutralTimer,
    clearNeutralTimer,
    setToolClearTimer,
    clearToolClearTimer,
  ])
}
