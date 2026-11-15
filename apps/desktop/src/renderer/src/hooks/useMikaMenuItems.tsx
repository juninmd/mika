import { History, MessageCircle, MessageCirclePlus, Mic, Minimize2, Pin, Settings2, Trash2 } from 'lucide-react'
import { useMemo } from 'react'
import type { ContextMenuItem } from '../components/MikaContextMenu'
import { useDockActions } from './useDockActions'
import { useI18n } from './useI18n'
import { setMiniMode } from './useMiniMode'

/** The mini window is 240 px tall: only what fits, and the way back out. */
const MINI_MENU = new Set(['mini', 'mic', 'settings'])

interface MikaMenuActions {
  mini: boolean
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
  const {
    mini,
    pinned,
    micEnabled,
    onTogglePin,
    onOpenChat,
    onOpenHistory,
    onToggleMic,
    onClearHistory,
    onSendMessage,
  } = actions

  const items = useMemo(
    (): ContextMenuItem[] => [
      {
        id: 'chat',
        label: t('shell.menu.chat'),
        hint: 'Ctrl+Space',
        icon: <MessageCircle size={14} />,
        onSelect: onOpenChat,
      },
      {
        id: 'topic',
        label: t('shell.menu.topic'),
        icon: <MessageCirclePlus size={14} />,
        onSelect: () => onSendMessage(t('shell.menu.topicPrompt')),
      },
      {
        id: 'history',
        label: t('shell.menu.history'),
        icon: <History size={14} />,
        onSelect: onOpenHistory,
        separatorBefore: true,
      },
      {
        id: 'mic',
        label: t('shell.menu.mic'),
        hint: 'F9',
        checked: micEnabled,
        icon: <Mic size={14} />,
        onSelect: onToggleMic,
      },
      {
        id: 'pin',
        label: t('shell.menu.pin'),
        checked: pinned,
        icon: <Pin size={14} />,
        onSelect: onTogglePin,
      },
      {
        id: 'mini',
        label: t('shell.menu.miniMode'),
        checked: mini,
        icon: <Minimize2 size={14} />,
        onSelect: () => setMiniMode(!mini),
      },
      {
        id: 'settings',
        label: t('shell.menu.settings'),
        icon: <Settings2 size={14} />,
        onSelect: () => void openSettings(),
        separatorBefore: true,
      },
      {
        id: 'clear',
        label: t('shell.menu.clearHistory'),
        icon: <Trash2 size={14} />,
        onSelect: onClearHistory,
        confirmLabel: t('shell.menu.clearHistoryArmed'),
        destructive: true,
      },
    ],
    [
      mini,
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
  return useMemo(() => (mini ? items.filter((item) => MINI_MENU.has(item.id)) : items), [mini, items])
}
