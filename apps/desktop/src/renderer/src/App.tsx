import { memo, useCallback, useState } from 'react'
import { AppAvatarLayer, AppDialogs } from './AppLayers'
import { BootOverlay } from './components/app/BootOverlay'
import { ChatComposer } from './components/app/ChatComposer'
import { HudLayer } from './components/app/HudLayer'
import { PanelLayer } from './components/app/PanelLayer'
import { StageBackdrop } from './components/app/StageBackdrop'
import { CommandPalette } from './components/CommandPalette'
import { DockContainer } from './components/DockContainer'
import { EffectOverlay } from './components/effects/EffectOverlay'
import { LegalGate } from './components/LegalGate'
import { MikaContextMenu } from './components/MikaContextMenu'
import { NeuralWheel } from './components/NeuralWheel'
import { NotificationContainer } from './components/NotificationToast'
import { ProactiveThoughtBubble } from './components/ProactiveThoughtBubble'
import { useAppShell } from './hooks/useAppShell'
import { useClickThrough } from './hooks/useClickThrough'
import { useHudLayout } from './hooks/useHudLayout'
import { useLegalGate } from './hooks/useLegalGate'
import { useMikaContextMenu } from './hooks/useMikaContextMenu'
import { useMikaMenuItems } from './hooks/useMikaMenuItems'
import { useNeuralWheel } from './hooks/useNeuralWheel'
import { useProviderHealthIndicator } from './hooks/useProviderHealthIndicator'
import { useRuntimeEvents } from './hooks/useRuntimeEvents'
import { useTheme } from './hooks/useTheme'
import { useToolApprovalGate } from './hooks/useToolApprovalGate'
import { useUpdateChecker } from './hooks/useUpdateChecker'
import { layerStyle } from './lib/layer-styles'
import { handleWindowDragPointerDown } from './lib/window-drag'
import { useChatStore } from './store/useChatStore'

export const App = memo(function App() {
  useTheme()
  useClickThrough()
  const { available: updateAvailable } = useUpdateChecker()
  const providerDown = useProviderHealthIndicator()
  const { isOpen: wheelOpen, position: wheelPos, openWheel, closeWheel } = useNeuralWheel()
  const { pending: approvalPending, approve, deny } = useToolApprovalGate()
  const legalGate = useLegalGate()

  const [showTelemetryOptIn, setShowTelemetryOptIn] = useState(false)

  useRuntimeEvents(setShowTelemetryOptIn)

  const shell = useAppShell()
  const {
    input,
    setInput,
    appStatus,
    micEnabled,
    sttEnabled,
    inputMode,
    vadIsSpeaking,
    speechHeld,
    pinned,
    showInput,
    showHistory,
    toggleOverlay,
    closeOverlays,
    messages,
    mikaReady,
    backgroundMode,
    backgroundSolidColor,
    capabilities,
    activeTool,
    ttsIsPlaying,
    voiceLang,
    notifications,
    dismiss,
    handleSend,
    sendMessage,
    handleToggleMic,
    handleSetMic,
    handleTogglePin,
    handleClearHistory,
    stopTTS,
    expression,
    volumeRef,
  } = shell
  const hudLayout = useHudLayout()

  const retryLastMessage = useCallback(() => {
    const lastUserMsg = [...useChatStore.getState().messages].reverse().find((m) => m.role === 'user')
    if (lastUserMsg) sendMessage(lastUserMsg.content)
  }, [sendMessage])

  const handleOpenInput = useCallback(() => toggleOverlay('input'), [toggleOverlay])
  const handleOpenHistory = useCallback(() => toggleOverlay('history'), [toggleOverlay])

  const { anchor: menuAnchor, open: openContextMenu, close: closeContextMenu } = useMikaContextMenu()
  const menuItems = useMikaMenuItems({
    pinned,
    micEnabled,
    onTogglePin: handleTogglePin,
    onOpenChat: handleOpenInput,
    onOpenHistory: handleOpenHistory,
    onToggleMic: handleToggleMic,
    onClearHistory: handleClearHistory,
    onSendMessage: sendMessage,
  })

  const isTransparentMode = backgroundMode === 'transparent'

  return (
    <>
      {/* biome-ignore lint/a11y/noStaticElementInteractions: full-window background shell — a right-click surface for the app's own context menu, not a focusable/keyboard-operable widget. */}
      <div
        className="relative h-screen w-screen overflow-clip select-none bg-transparent text-text-main"
        inert={legalGate.needed || legalGate.failed}
        onContextMenu={openContextMenu}
        onPointerDown={(e) => handleWindowDragPointerDown(e.nativeEvent)}
      >
        <div className="electron-drag absolute left-0 right-0 top-0 h-10" style={layerStyle('hud')} />

        <StageBackdrop backgroundMode={backgroundMode} backgroundSolidColor={backgroundSolidColor} />

        <EffectOverlay />

        {!mikaReady && !isTransparentMode && <BootOverlay />}

        <AppAvatarLayer shell={shell} />

        <HudLayer
          layout={hudLayout}
          backgroundMode={backgroundMode}
          appStatus={appStatus}
          activeTool={activeTool}
          micEnabled={micEnabled}
          vadIsSpeaking={vadIsSpeaking}
          speechHeld={speechHeld}
          ttsIsPlaying={ttsIsPlaying}
          volumeRef={volumeRef}
          expression={expression}
          providerDown={providerDown}
          visionEnabled={capabilities.canSeeScreen}
          memoryEnabled={capabilities.canLearn}
          mikaReady={mikaReady}
          onToggleMic={handleToggleMic}
          retryLastMessage={retryLastMessage}
        />

        <PanelLayer
          showHistory={showHistory}
          messages={messages}
          closeOverlays={closeOverlays}
          handleClearHistory={handleClearHistory}
          onRegenerateLast={retryLastMessage}
        />

        {showInput && (
          <div className="absolute inset-0 pointer-events-none" style={layerStyle('chat')}>
            <div className="pointer-events-auto">
              <ChatComposer
                value={input}
                onChange={setInput}
                onSend={handleSend}
                onClose={closeOverlays}
                speechLang={voiceLang}
              />
            </div>
          </div>
        )}

        <div className="absolute inset-x-0 bottom-0 flex justify-center" style={layerStyle('dock')}>
          <DockContainer
            backgroundMode={backgroundMode}
            appStatus={appStatus}
            micEnabled={micEnabled}
            vadIsSpeaking={vadIsSpeaking}
            pinned={pinned}
            showInput={showInput}
            showHistory={showHistory}
            onToggleMic={handleToggleMic}
            onSetMic={handleSetMic}
            onOpenInput={handleOpenInput}
            onOpenHistory={handleOpenHistory}
            onOpenWheel={() => openWheel()}
            ttsIsPlaying={ttsIsPlaying}
            onStopSpeech={stopTTS}
            sttEnabled={sttEnabled}
            inputMode={inputMode}
            updateAvailable={updateAvailable}
            providerDown={providerDown}
          />
        </div>

        <NotificationContainer notifications={notifications} onDismiss={dismiss} anchor={hudLayout.toasts} />
        <ProactiveThoughtBubble anchor={hudLayout.toasts} />
        <CommandPalette />

        <NeuralWheel
          isOpen={wheelOpen}
          onClose={closeWheel}
          position={wheelPos}
          onCommand={sendMessage}
          micEnabled={micEnabled}
          onToggleMic={handleToggleMic}
          pinned={pinned}
          onTogglePin={handleTogglePin}
          onOpenChat={handleOpenInput}
          onClearChat={handleClearHistory}
          ttsIsPlaying={ttsIsPlaying}
          onStopTTS={stopTTS}
        />

        <MikaContextMenu anchor={menuAnchor} items={menuItems} onClose={closeContextMenu} />

        <AppDialogs
          showTelemetryOptIn={showTelemetryOptIn}
          setShowTelemetryOptIn={setShowTelemetryOptIn}
          approvalPending={approvalPending}
          approve={approve}
          deny={deny}
          approvalAnchor={{
            centerX: hudLayout.bubbleRect.x + hudLayout.bubbleRect.width / 2,
            top: hudLayout.bubble.y,
          }}
        />
      </div>
      <LegalGate gate={legalGate} />
    </>
  )
})
