import { memo, type RefObject, useCallback, useEffect, useMemo, useState } from 'react'
import { useGameSession } from '../../hooks/useGameSession'
import { useMicHearing } from '../../hooks/useMicHearing'
import { useTransientFlag } from '../../hooks/useTransientFlag'
import { useChatStore } from '../../store/useChatStore'
import { ActiveEffectChip } from '../effects/ActiveEffectChip'
import { GameSessionHud } from '../hud/GameSessionHud'
import type { HudLayout } from '../hud/hud-layout'
import { type BackgroundMode, hudPolicy } from '../hud/hud-policy'
import { PresenceOrb } from '../hud/PresenceOrb'
import { derivePresence } from '../hud/presence-state'
import { MoodHUD } from '../MoodHUD'
import { PluginSlot } from '../PluginSlot'
import { SensesMonitor } from '../SensesMonitor'
import { SpeechBubble } from '../SpeechBubble'
import { ThinkingHUD } from '../ThinkingHUD'
import { TokenCostBadge } from '../TokenCostBadge'
import type { ActiveToolState, AppStatus } from './app-shell.types'

const TRANSCRIPT_HOLD_MS = 2500

// Shows the last transcript for 2.5s: fired before we know if it becomes a turn, so the user sees they were heard either way.
function useLastTranscript(): string | undefined {
  const [text, setText] = useState('')
  const [visible, show] = useTransientFlag(TRANSCRIPT_HOLD_MS)

  useEffect(() => {
    const onTranscript = (event: Event) => {
      const spoken = (event as CustomEvent<string>).detail?.trim()
      if (!spoken) return
      setText(spoken)
      show()
    }
    window.addEventListener('stt-transcript', onTranscript)
    return () => window.removeEventListener('stt-transcript', onTranscript)
  }, [show])

  return visible ? text : undefined
}

interface HudLayerProps {
  layout: HudLayout
  backgroundMode: BackgroundMode
  appStatus: AppStatus
  activeTool: ActiveToolState
  micEnabled: boolean
  vadIsSpeaking: boolean
  /** Speech was heard during a turn and is queued — the presence turns amber instead of going quiet. */
  speechHeld: boolean
  ttsIsPlaying: boolean
  /** Lip-sync level of the voice being played (0..1); the speaking bars read it per frame. */
  volumeRef: RefObject<number>
  expression: string
  providerDown: boolean
  visionEnabled: boolean
  memoryEnabled: boolean
  mikaReady: boolean
  onToggleMic: () => void
  retryLastMessage: () => void
}

/**
 * Main-window HUD, laid out around the avatar (`hud/hud-layout.ts`) and thinned per background
 * mode (`hud/hud-policy.ts`): presence orb beside her head, speech bubble beside or under it, the
 * game session chip on top. Nothing here covers her face.
 */
export const HudLayer = memo(function HudLayer(props: HudLayerProps) {
  const { layout, backgroundMode, appStatus, activeTool, micEnabled, vadIsSpeaking, speechHeld, ttsIsPlaying } = props
  const policy = hudPolicy(backgroundMode)
  const bubbleVisible = useChatStore((s) => s.bubbleVisible)
  const turnSteps = useChatStore((s) => s.turnSteps)
  const transcript = useLastTranscript()
  // Not `vadIsSpeaking`: that one means "a capture session is open", true the whole time the mic is.
  const hearing = useMicHearing(micEnabled)
  const session = useGameSession()

  const presence = useMemo(
    () =>
      derivePresence({
        appStatus,
        providerDown: props.providerDown,
        micEnabled,
        hearing,
        speechHeld,
        ttsIsPlaying,
        gaming: session !== null,
      }),
    [appStatus, props.providerDown, micEnabled, hearing, speechHeld, ttsIsPlaying, session],
  )

  const { visionEnabled, memoryEnabled, mikaReady } = props
  const renderDetails = useCallback(
    () => (
      <>
        {mikaReady && (
          <SensesMonitor
            micEnabled={micEnabled}
            vadIsSpeaking={vadIsSpeaking}
            ttsIsPlaying={ttsIsPlaying}
            visionEnabled={visionEnabled}
            memoryEnabled={memoryEnabled}
            appStatus={appStatus}
            activeTool={activeTool}
          />
        )}
        <MoodHUD />
        <TokenCostBadge />
      </>
    ),
    [mikaReady, micEnabled, vadIsSpeaking, ttsIsPlaying, visionEnabled, memoryEnabled, appStatus, activeTool],
  )

  const bubble = layout.bubble
  // The step trace is telemetry: it only exists where the window owns its pixels (semi/solid).
  const traceVisible =
    policy.density !== 'minimal' && ((appStatus === 'thinking' && !bubbleVisible) || activeTool !== null)
  const traceStyle = {
    left: bubble.side === 'below' ? bubble.x - bubble.maxWidth / 2 : bubble.x,
    top: bubble.y,
    width: bubble.maxWidth,
  }

  return (
    <div className="absolute inset-0 pointer-events-none" style={{ zIndex: 'var(--layer-hud)' }}>
      <PluginSlot name="hud-top" />

      {session && policy.gameHud !== 'badge' && (
        <GameSessionHud session={session} anchor={layout.gameChip} variant={policy.gameHud} />
      )}

      <PresenceOrb
        presence={presence}
        policy={policy}
        anchor={layout.presence}
        micEnabled={micEnabled}
        volumeRef={props.volumeRef}
        onToggleMic={props.onToggleMic}
        gameName={session?.gameName}
        renderDetails={renderDetails}
        pinned={<ActiveEffectChip />}
        bubbleShown={bubbleVisible || traceVisible}
      />

      <SpeechBubble
        status={appStatus}
        activeTool={activeTool}
        onRetry={appStatus === 'error' ? props.retryLastMessage : undefined}
        anchor={bubble}
        expression={props.expression}
      />

      {policy.density !== 'minimal' && (
        <div className="pointer-events-none absolute flex justify-center" style={traceStyle}>
          <ThinkingHUD visible={traceVisible} activeTool={activeTool?.name} steps={turnSteps} transcript={transcript} />
        </div>
      )}
    </div>
  )
})
