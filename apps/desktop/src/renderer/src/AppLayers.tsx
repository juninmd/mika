import { AnimatePresence } from 'framer-motion'
import { lazy, Suspense } from 'react'
import { AvatarAura } from './components/app/AvatarAura'
import { AvatarStageBoundary } from './components/app/AvatarStageBoundary'
import { isVrmModelPath } from './components/avatar/vrm-look'
import { CrisisResourcesCard } from './components/CrisisResourcesCard'
import { EmojiReactionLayer } from './components/EmojiReaction'
import { PluginSlot } from './components/PluginSlot'
import { TelemetryOptIn } from './components/TelemetryOptIn'
import { ToolApprovalDialog } from './components/ToolApprovalDialog'
import type { useAppShell } from './hooks/useAppShell'
import type { useToolApprovalGate } from './hooks/useToolApprovalGate'
import { layerStyle } from './lib/layer-styles'

const Live2DCanvas = lazy(() => import('./components/Live2DCanvas').then((m) => ({ default: m.Live2DCanvas })))
const VrmCanvas = lazy(() => import('./components/VrmCanvas').then((m) => ({ default: m.VrmCanvas })))

export function AppAvatarLayer({ shell }: { shell: ReturnType<typeof useAppShell> }) {
  const {
    appStatus,
    expression,
    mouseTracking,
    live2dModelPath,
    live2dReady,
    volumeRef,
    vowelsRef,
    mousePosRef,
    reactions,
    ttsIsPlaying,
    handleAvatarInteract,
    expressionIntensity,
  } = shell
  // A extensão do modelo decide o canvas: .vrm → three-vrm, senão Live2D.
  const AvatarCanvas = isVrmModelPath(live2dModelPath) ? VrmCanvas : Live2DCanvas
  // `key` on the boundary: picking another skin remounts it, so one failed model doesn't blank the stage for good.

  return (
    <div className="absolute inset-0" style={layerStyle('avatar')}>
      {live2dReady && <AvatarAura status={appStatus} />}
      {live2dReady && (
        <AvatarStageBoundary key={live2dModelPath}>
          <Suspense fallback={null}>
            <AvatarCanvas
              expression={expression}
              intensity={expressionIntensity}
              modelPath={live2dModelPath}
              isSpeaking={appStatus === 'speaking' || ttsIsPlaying}
              volumeRef={volumeRef}
              vowelsRef={vowelsRef}
              mouseRef={mouseTracking ? mousePosRef : undefined}
              onAvatarInteract={handleAvatarInteract}
            />
          </Suspense>
        </AvatarStageBoundary>
      )}
      <EmojiReactionLayer reactions={reactions} />
      <PluginSlot name="avatar-overlay" />
    </div>
  )
}

type AppDialogsProps = {
  showTelemetryOptIn: boolean
  setShowTelemetryOptIn: (show: boolean) => void
} & Pick<ReturnType<typeof useToolApprovalGate>, 'approve' | 'deny'> & {
    approvalPending: ReturnType<typeof useToolApprovalGate>['pending']
    /** The bubble slot: the approval card opens where her reply would, not over her face. */
    approvalAnchor: { centerX: number; top: number }
  }

export function AppDialogs({
  showTelemetryOptIn,
  setShowTelemetryOptIn,
  approvalPending,
  approve,
  deny,
  approvalAnchor,
}: AppDialogsProps) {
  return (
    <>
      {showTelemetryOptIn && <TelemetryOptIn onDismiss={() => setShowTelemetryOptIn(false)} />}
      <CrisisResourcesCard />

      <AnimatePresence mode="wait">
        {approvalPending && (
          <ToolApprovalDialog
            key={`approval-${approvalPending.toolCallId}`}
            toolCallId={approvalPending.toolCallId}
            toolName={approvalPending.toolName}
            args={approvalPending.args}
            onApprove={approve}
            onDeny={deny}
            anchor={approvalAnchor}
          />
        )}
      </AnimatePresence>
    </>
  )
}
