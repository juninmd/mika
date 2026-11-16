import { Activity, Eraser, Gamepad2, MessageSquareText, Mic, MicOff, Pin, PinOff, ScanEye, VolumeX } from 'lucide-react'
import { useMemo } from 'react'
import { useI18n } from '../hooks/useI18n'
import {
  runChatAction,
  runClearAction,
  runMicAction,
  runMuteAction,
  runPinAction,
  runSystemAction,
  runWatchAction,
} from './neural-wheel-actions'

export interface WheelItem {
  id: string
  label: string
  subLabel?: string
  icon: React.ReactNode
  color: 'cyan' | 'purple' | 'mint' | 'coral' | 'amber'
  kind: 'action' | 'submenu'
  prompt?: string
  onExecute?: () => void
}

type T = ReturnType<typeof useI18n>['t']

function createMicItem(micEnabled: boolean, t: T, onToggleMic?: () => void, onClose?: () => void): WheelItem {
  return {
    id: 'mic',
    label: t(micEnabled ? 'shell.neuralWheel.item.muteMic' : 'shell.neuralWheel.item.unmuteMic'),
    subLabel: t(micEnabled ? 'shell.neuralWheel.item.micOn' : 'shell.neuralWheel.item.micMuted'),
    icon: micEnabled ? <Mic size={22} /> : <MicOff size={22} />,
    kind: 'action',
    color: micEnabled ? 'mint' : 'coral',
    onExecute: () => runMicAction(micEnabled, onToggleMic, onClose),
  }
}

function createMuteItem(ttsIsPlaying: boolean, t: T, onStopTTS?: () => void, onClose?: () => void): WheelItem {
  return {
    id: 'mute',
    label: t('shell.neuralWheel.item.muteMika'),
    subLabel: t(ttsIsPlaying ? 'shell.neuralWheel.item.speakingNow' : 'shell.neuralWheel.item.stopAudio'),
    icon: <VolumeX size={22} className={ttsIsPlaying ? 'animate-pulse' : undefined} />,
    kind: 'action',
    color: ttsIsPlaying ? 'coral' : 'amber',
    onExecute: () => runMuteAction(onStopTTS, onClose),
  }
}

function createPinItem(pinned: boolean, t: T, onTogglePin?: () => void, onClose?: () => void): WheelItem {
  return {
    id: 'pin',
    label: t(pinned ? 'shell.neuralWheel.item.unpin' : 'shell.neuralWheel.item.pinWindow'),
    subLabel: t(pinned ? 'shell.neuralWheel.item.alwaysOnTop' : 'shell.neuralWheel.item.windowFloating'),
    icon: pinned ? <PinOff size={22} /> : <Pin size={22} />,
    kind: 'action',
    color: pinned ? 'amber' : 'purple',
    onExecute: () => runPinAction(pinned, onTogglePin, onClose),
  }
}

export interface NeuralWheelItemsInput {
  onClose: () => void
  onCommand: (prompt: string) => void
  micEnabled: boolean
  onToggleMic?: () => void
  pinned: boolean
  onTogglePin?: () => void
  onOpenChat?: () => void
  onClearChat?: () => void
  ttsIsPlaying: boolean
  onStopTTS?: () => void
}

export function useNeuralWheelItems({
  onClose,
  onCommand,
  micEnabled,
  onToggleMic,
  pinned,
  onTogglePin,
  onOpenChat,
  onClearChat,
  ttsIsPlaying,
  onStopTTS,
}: NeuralWheelItemsInput): WheelItem[] {
  const { t } = useI18n()
  return useMemo<WheelItem[]>(() => {
    return [
      {
        id: 'games',
        label: t('shell.neuralWheel.item.myGames'),
        subLabel: 'Steam',
        icon: <Gamepad2 size={22} />,
        kind: 'submenu',
        color: 'purple',
      },
      {
        id: 'watch',
        label: t('shell.neuralWheel.item.watchScreen'),
        subLabel: t('shell.neuralWheel.item.watchScreenSub'),
        icon: <ScanEye size={22} />,
        kind: 'action',
        color: 'cyan',
        onExecute: () => runWatchAction(onCommand, onClose),
      },
      createMicItem(micEnabled, t, onToggleMic, onClose),
      createMuteItem(ttsIsPlaying, t, onStopTTS, onClose),
      {
        id: 'chat',
        label: t('shell.neuralWheel.item.openChat'),
        subLabel: t('shell.neuralWheel.item.openChatSub'),
        icon: <MessageSquareText size={22} />,
        kind: 'action',
        color: 'cyan',
        onExecute: () => runChatAction(onOpenChat, onClose),
      },
      createPinItem(pinned, t, onTogglePin, onClose),
      {
        id: 'system',
        label: t('shell.neuralWheel.item.pcStatus'),
        subLabel: t('shell.neuralWheel.item.pcStatusSub'),
        icon: <Activity size={22} />,
        kind: 'action',
        color: 'mint',
        onExecute: () => runSystemAction(onCommand, onClose),
      },
      {
        id: 'clear',
        label: t('shell.neuralWheel.item.clearChat'),
        subLabel: t('shell.neuralWheel.item.clearChatSub'),
        icon: <Eraser size={22} />,
        kind: 'action',
        color: 'coral',
        onExecute: () => runClearAction(onClearChat, onClose),
      },
    ]
  }, [
    t,
    micEnabled,
    pinned,
    ttsIsPlaying,
    onToggleMic,
    onTogglePin,
    onOpenChat,
    onClearChat,
    onStopTTS,
    onCommand,
    onClose,
  ])
}
