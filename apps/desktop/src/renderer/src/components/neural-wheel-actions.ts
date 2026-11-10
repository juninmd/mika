import { t } from '@mika/sdk/i18n'
import { getElectronApi } from '../lib/shell-api'
import { useChatStore } from '../store/useChatStore'
import { useUIStore } from '../store/useUIStore'

export function runWatchAction(onCommand: (prompt: string) => void, onClose: () => void) {
  useUIStore
    .getState()
    .addNotification('info', t('shell.neuralWheel.watch.analyzing'), t('shell.neuralWheel.notice.vision'))
  const api = getElectronApi()
  if (api) {
    void api.runtime
      .executeTool('vision_capture', { action: 'screenshot' })
      .then((result) => {
        const res = result as { success?: boolean; data?: { analysis?: string } }
        if (res?.success && res.data?.analysis) {
          useUIStore
            .getState()
            .addNotification('success', t('shell.neuralWheel.watch.done'), t('shell.neuralWheel.notice.vision'))
        }
      })
      .catch(() => {
        useUIStore
          .getState()
          .addNotification('warning', t('shell.neuralWheel.watch.failed'), t('shell.neuralWheel.notice.vision'))
      })
  }
  onCommand(t('shell.neuralWheel.watch.prompt'))
  onClose()
}

export function runMicAction(micEnabled: boolean, onToggleMic?: () => void, onClose?: () => void) {
  onToggleMic?.()
  const willBeActive = !micEnabled
  useUIStore
    .getState()
    .addNotification(
      willBeActive ? 'success' : 'info',
      t(willBeActive ? 'shell.neuralWheel.mic.on' : 'shell.neuralWheel.mic.off'),
      t('shell.neuralWheel.notice.microphone'),
    )
  onClose?.()
}

export function runMuteAction(onStopTTS?: () => void, onClose?: () => void) {
  onStopTTS?.()
  useChatStore.getState().setBubbleVisible(false)
  useChatStore.getState().setAppStatus('idle')
  useUIStore.getState().addNotification('info', t('shell.neuralWheel.mute.done'), t('shell.neuralWheel.notice.audio'))
  onClose?.()
}

export function runChatAction(onOpenChat?: () => void, onClose?: () => void) {
  if (onOpenChat) onOpenChat()
  else useUIStore.getState().setShowInput(true)
  onClose?.()
}

export function runPinAction(pinned: boolean, onTogglePin?: () => void, onClose?: () => void) {
  if (onTogglePin) {
    onTogglePin()
  } else {
    useUIStore.getState().setPinned(!useUIStore.getState().pinned)
    const api = getElectronApi()
    if (api?.shell.window) void api.shell.window.toggleAlwaysOnTop()
  }
  const willBePinned = !pinned
  useUIStore
    .getState()
    .addNotification(
      willBePinned ? 'success' : 'info',
      t(willBePinned ? 'shell.neuralWheel.pin.on' : 'shell.neuralWheel.pin.off'),
      t('shell.neuralWheel.notice.window'),
    )
  onClose?.()
}

export function runSystemAction(onCommand: (prompt: string) => void, onClose: () => void) {
  const api = getElectronApi()
  void Promise.all([
    api?.runtime.getPowerState().catch(() => null),
    api?.runtime.executeTool('desktop', { action: 'process' }).catch(() => null),
  ]).then(([power, procResult]) => {
    const isBattery = power?.onBatteryPower === true
    const processes =
      (procResult as { data?: { processes?: Array<{ name: string; memoryMB: number }> } })?.data?.processes ?? []
    const top3 = processes
      .slice(0, 3)
      .map((p) => `${p.name} (${p.memoryMB}MB)`)
      .join(', ')
    const source = t(isBattery ? 'shell.neuralWheel.system.battery' : 'shell.neuralWheel.system.ac')
    const powerLine = t('shell.neuralWheel.system.power', { source })
    const msg = top3 ? `${powerLine} | ${t('shell.neuralWheel.system.topProcesses', { list: top3 })}` : powerLine
    useUIStore.getState().addNotification('info', msg, t('shell.neuralWheel.notice.system'))
  })
  onCommand(t('shell.neuralWheel.system.prompt'))
  onClose()
}

export function runClearAction(onClearChat?: () => void, onClose?: () => void) {
  if (onClearChat) {
    onClearChat()
  } else {
    const api = getElectronApi()
    if (api) void api.runtime.clearChatHistory()
    useChatStore.getState().setMessages([])
  }
  useUIStore
    .getState()
    .addNotification('success', t('shell.neuralWheel.clear.done'), t('shell.neuralWheel.notice.chat'))
  onClose?.()
}
