import { t } from '@mika/sdk/i18n'
import { useEffect, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useEmojiReactions } from '../components/EmojiReaction'
import { getElectronApi } from '../lib/shell-api'
import { useChatStore } from '../store/useChatStore'
import { useTrackingStore } from '../store/useTrackingStore'
import { useUIStore } from '../store/useUIStore'
import { needsNeuralVadFeed } from './shell-config-sync'
import { useAudioPlayback } from './useAudioPlayback'
import { useDesktopShellBootstrap } from './useDesktopShellBootstrap'
import { useGlobalShortcuts } from './useGlobalShortcuts'
import { useMikaEvents } from './useMikaEvents'
import { useMouseTracking } from './useMouseTracking'
import { useShellEvents } from './useShellEvents'
import { useShellMessaging } from './useShellMessaging'
import { useShellMic } from './useShellMic'
import { useShellStatusSync } from './useShellStatusSync'
import { useShellUiActions } from './useShellUiActions'
import { useShellUiState } from './useShellUiState'
import { useShellVoice } from './useShellVoice'
import { useShellVoiceConfig } from './useShellVoiceConfig'
import { useTimeoutRef } from './useTransientFlag'

export function useAppShell() {
  const eAPIRef = useRef<ReturnType<typeof getElectronApi> | null>(null)
  eAPIRef.current ??= getElectronApi()
  const eAPI = eAPIRef.current

  const { messages, setMessages, setBubble, setBubbleVisible, appStatus, setAppStatus, activeTool, sessionUsage } =
    useChatStore(
      useShallow((s) => ({
        messages: s.messages,
        setMessages: s.setMessages,
        setBubble: s.setBubble,
        setBubbleVisible: s.setBubbleVisible,
        appStatus: s.appStatus,
        setAppStatus: s.setAppStatus,
        activeTool: s.activeTool,
        sessionUsage: s.sessionUsage,
      })),
    )

  const showInput = useUIStore((s) => s.showInput)
  const setShowInput = useUIStore((s) => s.setShowInput)
  const showHistory = useUIStore((s) => s.showHistory)
  const setShowHistory = useUIStore((s) => s.setShowHistory)
  const pinned = useUIStore((s) => s.pinned)
  const setPinned = useUIStore((s) => s.setPinned)
  const toggleOverlay = useUIStore((s) => s.toggleOverlay)
  const closeOverlays = useUIStore((s) => s.closeOverlays)
  const addNotification = useUIStore((s) => s.addNotification)
  const notifications = useUIStore((s) => s.notifications)
  const dismissNotification = useUIStore((s) => s.dismissNotification)

  const mouseTracking = useTrackingStore((s) => s.mouseTrackingEnabled)

  const uiState = useShellUiState()
  const { input, setInput, expression, expressionIntensity, setExpression, mousePosRef } = uiState
  const { live2dModelPath, setLive2dModelPath, backgroundMode, setBackgroundMode } = uiState
  const { backgroundSolidColor, setBackgroundSolidColor, capabilities, setCapabilities } = uiState
  const { mikaReady, setMikaReady, volumeRef, ttsVolumeRef } = uiState
  // Single expression-reset timer shared across response/error/interaction/vision paths to avoid races on `expression`.
  const { set: scheduleExpressionReset } = useTimeoutRef()

  const { reactions, spawnReaction } = useEmojiReactions()

  const {
    voiceConfigLoaded,
    sttEnabled,
    inputMode,
    voiceLang,
    asrProvider,
    autoMuteDuringTts,
    interruptionEnabled,
    vadThreshold,
    vadSilenceMs,
    vadEndpointingEnabled,
    vadEngine,
    vadIgnoreTyping,
    offlineMode,
    inputDeviceId,
  } = useShellVoiceConfig(eAPI, ttsVolumeRef)

  const { micEnabled, setMicEnabled, handleToggleMic, handleSetMic } = useShellMic({
    eAPI,
    voiceConfigLoaded,
    sttEnabled,
    inputMode,
  })

  // Barge-in opens the mic too: with the mic toggled off it kept listening (and "being interrupted").
  const {
    enqueue: enqueueTTS,
    stopAll: stopTTS,
    isPlaying: ttsIsPlaying,
    vowelsRef,
  } = useAudioPlayback(
    volumeRef,
    needsNeuralVadFeed({ interruptionEnabled, vadEngine, vadIgnoreTyping, micEnabled }),
    ttsVolumeRef,
    inputDeviceId,
  )

  const { sendMessage, doClearHistory, isSendingRef, speechHeld, holdSpeech } = useShellMessaging({
    eAPI,
    messages,
    setMessages,
    setBubble,
    setBubbleVisible,
    setAppStatus,
    addNotification,
    setExpression,
    scheduleExpressionReset,
  })

  const voiceParamsA = {
    voiceLang,
    asrProvider,
    inputDeviceId,
    vadThreshold,
    vadSilenceMs,
    vadEndpointingEnabled,
    offlineMode,
    micEnabled,
  }
  const voiceParamsB = { eAPI, setMicEnabled, appStatus, ttsIsPlaying, stopTTS, isSendingRef, holdSpeech }
  const voiceParams = { ...voiceParamsA, ...voiceParamsB, sendMessage, addNotification }
  const { vadIsSpeaking, setSpeechPaused } = useShellVoice(voiceParams)

  const uiActionsParamsA = { eAPI, input, setInput, showInput, closeOverlays, stopTTS, sendMessage }
  const uiActionsParamsB = { setPinned, setMicEnabled, setExpression, spawnReaction }
  const uiActionsParams = { ...uiActionsParamsA, ...uiActionsParamsB, scheduleExpressionReset, mousePosRef }
  const { closeOverlaysWrapper, handleSend, handleTogglePin, handleAvatarInteract, handleMouseTrack } =
    useShellUiActions(uiActionsParams)

  useShellStatusSync({
    appStatus,
    setAppStatus,
    ttsIsPlaying,
    offlineMode,
    vadIsSpeaking,
    micEnabled,
  })

  useEffect(() => {
    setSpeechPaused(ttsIsPlaying && autoMuteDuringTts)
  }, [ttsIsPlaying, autoMuteDuringTts, setSpeechPaused])

  useGlobalShortcuts({
    onToggleInput: () => toggleOverlay('input'),
    onToggleMic: handleToggleMic,
    onToggleHistory: () => toggleOverlay('history'),
    onToggleFullscreen: () => void eAPI.shell.window?.toggleFullscreen?.(),
    onHide: closeOverlaysWrapper,
    // Same path as holding the dock mic button (`CommandDock` pttMode → onSetMic).
    onPushToTalk: inputMode === 'ptt' && sttEnabled ? handleSetMic : undefined,
  })

  useDesktopShellBootstrap({
    setBackgroundMode,
    setBackgroundSolidColor,
    setCapabilities,
    setLive2dModelPath,
    setMikaReady,
  })

  useMikaEvents({
    enqueueTTS,
    setExpression,
    stopTTS,

    onTTSError: (message) => addNotification('error', message, t('shell.neuralWheel.notice.audio')),
  })

  const { live2dReady } = useShellEvents({
    platform: eAPI.platform,
    spawnReaction,
    setExpression,
    scheduleExpressionReset,
  })

  useMouseTracking(mouseTracking, handleMouseTrack)

  // Grouped so the final return stays a single spread instead of one 40-line property list.
  const composerApi = { input, setInput, sendMessage, handleSend }
  const voiceApi = { micEnabled, sttEnabled, inputMode, inputDeviceId, vadIsSpeaking, speechHeld, voiceLang }
  const voiceHandlers = { handleToggleMic, handleSetMic, ttsIsPlaying, stopTTS }
  const chatApi = { appStatus, expression, expressionIntensity, messages, activeTool, sessionUsage }
  const overlayApi = { mouseTracking, pinned, showInput, setShowInput, showHistory, setShowHistory, toggleOverlay }
  const avatarApi = { mikaReady, live2dModelPath, backgroundMode, backgroundSolidColor, capabilities, live2dReady }
  const avatarRefs = { volumeRef, vowelsRef, mousePosRef, reactions, handleAvatarInteract }

  return {
    ...composerApi,
    ...voiceApi,
    ...voiceHandlers,
    ...chatApi,
    ...overlayApi,
    ...avatarApi,
    ...avatarRefs,
    closeOverlays: closeOverlaysWrapper,
    notifications,
    dismiss: dismissNotification,
    handleTogglePin,
    handleClearHistory: doClearHistory,
  }
}
