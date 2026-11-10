import { useMemo } from 'react'
import {
  HistoryIcon,
  MessageIcon,
  MicrophoneIcon,
  PinIcon,
  SettingsIcon,
  SparkIcon,
  TrashIcon,
} from '../components/Icons'
import type { ContextMenuItem } from '../components/MikaContextMenu'
import { useDockActions } from './useDockActions'
import { useI18n } from './useI18n'

interface MikaMenuActions {
  pinned: boolean
  micEnabled: boolean
  onTogglePin: () => void
  onOpenChat: () => void
  onOpenHistory: () => void
  onToggleMic: () => void
  onClearHistory: () => void
  onSendMessage: (text: string) => void
}

export function useMikaMenuItems(actions: MikaMenuActions): ContextMenuItem[] {
  const { t } = useI18n()
  const { openSettings } = useDockActions()
  const { pinned, micEnabled, onTogglePin, onOpenChat, onOpenHistory, onToggleMic, onClearHistory, onSendMessage } =
    actions

  return useMemo(
    () => [
      {
        id: 'chat',
        label: t('shell.menu.chat'),
        hint: 'Ctrl+Space',
        icon: <MessageIcon className="h-3.5 w-3.5" />,
        onSelect: onOpenChat,
      },
      {
        id: 'topic',
        label: t('shell.menu.topic'),
        icon: <SparkIcon className="h-3.5 w-3.5" />,
        onSelect: () => onSendMessage(t('shell.menu.topicPrompt')),
      },
      {
        id: 'history',
        label: t('shell.menu.history'),
        icon: <HistoryIcon className="h-3.5 w-3.5" />,
        onSelect: onOpenHistory,
        separatorBefore: true,
      },
      {
        id: 'mic',
        label: t('shell.menu.mic'),
        hint: 'F9',
        checked: micEnabled,
        icon: <MicrophoneIcon className="h-3.5 w-3.5" />,
        onSelect: onToggleMic,
      },
      {
        id: 'pin',
        label: t('shell.menu.pin'),
        checked: pinned,
        icon: <PinIcon className="h-3.5 w-3.5" />,
        onSelect: onTogglePin,
      },
      {
        id: 'settings',
        label: t('shell.menu.settings'),
        icon: <SettingsIcon className="h-3.5 w-3.5" />,
        onSelect: () => void openSettings(),
        separatorBefore: true,
      },
      {
        id: 'clear',
        label: t('shell.menu.clearHistory'),
        icon: <TrashIcon className="h-3.5 w-3.5" />,
        onSelect: onClearHistory,
        confirmLabel: t('shell.menu.clearHistoryArmed'),
        destructive: true,
      },
    ],
    [
      micEnabled,
      pinned,
      onOpenChat,
      onOpenHistory,
      onToggleMic,
      onTogglePin,
      onClearHistory,
      onSendMessage,
      openSettings,
      t,
    ],
  )
}
