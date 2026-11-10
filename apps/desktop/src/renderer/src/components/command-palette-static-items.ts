import { ConfigKey } from '@mika/sdk/contracts'
import { toDisplayError } from '@mika/sdk-ui'
import { Compass, History, Maximize, MessageSquare, Mic, Pin, Settings } from 'lucide-react'
import { type PaletteDeps, type PaletteItem, toolFailureMessage } from './command-palette-items'

export interface StaticPaletteDeps extends PaletteDeps {
  toggleOverlay: (overlay: 'input' | 'history') => void
  pinned: boolean
  setPinned: (pinned: boolean) => void
}

export function buildStaticItems({
  t,
  eAPI,
  close,
  addNotification,
  toggleOverlay,
  pinned,
  setPinned,
}: StaticPaletteDeps): PaletteItem[] {
  return [
    {
      id: 'chat',
      label: t('shell.palette.openChat'),
      icon: MessageSquare,
      description: 'Ctrl+Space',
      group: t('shell.palette.group.navigation'),
      onSelect: () => {
        close()
        toggleOverlay('input')
      },
    },
    {
      id: 'history',
      label: t('shell.palette.history'),
      icon: History,
      description: 'Ctrl+Shift+H',
      group: t('shell.palette.group.navigation'),
      onSelect: () => {
        close()
        toggleOverlay('history')
      },
    },
    {
      id: 'settings',
      label: t('shell.palette.settings'),
      icon: Settings,
      group: t('shell.palette.group.system'),
      onSelect: () => {
        close()
        eAPI?.shell?.openSettings?.()
      },
    },
    {
      id: 'reopen-wizard',
      label: t('shell.palette.reopenWizard'),
      description: t('shell.palette.reopenWizardHint'),
      icon: Compass,
      group: t('shell.palette.group.system'),
      onSelect: () => {
        close()
        // O shell observa setup.wizardCompleted e troca para o assistente; o main redimensiona a janela.
        eAPI?.runtime
          ?.saveSetting?.(ConfigKey.SetupWizardCompleted, false)
          .then((res) => {
            if (res && res.success === false) {
              addNotification(
                'error',
                toolFailureMessage(t, t('shell.palette.reopenWizard'), res.error),
                'reopen-wizard',
              )
            }
          })
          .catch((err: unknown) => {
            addNotification(
              'error',
              toolFailureMessage(t, t('shell.palette.reopenWizard'), toDisplayError(err)),
              'reopen-wizard',
            )
          })
      },
    },
    {
      id: 'toggle-pin',
      label: pinned ? t('shell.menu.pinOff') : t('shell.menu.pinOn'),
      icon: Pin,
      group: t('shell.palette.group.system'),
      onSelect: () => {
        close()
        void eAPI?.shell?.window?.toggleAlwaysOnTop?.().then((next) => {
          if (next !== undefined) setPinned(next)
        })
      },
    },
    {
      id: 'toggle-fullscreen',
      label: t('shell.palette.fullscreen'),
      description: 'F11',
      icon: Maximize,
      group: t('shell.palette.group.system'),
      onSelect: () => {
        close()
        void eAPI?.shell?.window?.toggleFullscreen?.()
      },
    },
    {
      id: 'toggle-mic',
      label: t('shell.menu.micOn'),
      description: 'Ctrl+M / F9',
      icon: Mic,
      group: t('shell.palette.group.system'),
      onSelect: () => {
        close()
        window.dispatchEvent(new KeyboardEvent('keydown', { ctrlKey: true, code: 'KeyM', bubbles: true }))
      },
    },
  ]
}
