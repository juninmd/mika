import { useEffect, useRef, useState } from 'react'
import { miniModeAction } from '../lib/mini-mode'
import { getElectronApi } from '../lib/shell-api'
import { subscribeWindowFlag } from '../lib/window-fullscreen'

export function setMiniMode(on: boolean): void {
  void getElectronApi().shell.window?.setMiniMode?.(on)
}

/** Modo mini REAL da janela do avatar (só a cabeça, canto inferior esquerdo); `null` até o backend responder. */
export function useMiniMode(): boolean | null {
  const [mini, setMini] = useState<boolean | null>(null)
  useEffect(() => {
    const api = getElectronApi().shell.window
    return subscribeWindowFlag(
      () => api?.isMiniMode?.(),
      (push) => api?.onMiniModeChange?.(({ mini }) => push(mini)),
      setMini,
    )
  }, [])
  return mini
}

/** Aplica `miniModeAction` na janela do avatar: fecha o que estava aberto ao entrar, sai quando algo precisa de espaço. */
export function useMiniModeGuard(mini: boolean, blocking: boolean, overlayOpen: boolean, closeOverlays: () => void) {
  const entered = useRef(false)
  useEffect(() => {
    const action = miniModeAction({ mini, entered: entered.current, blocking, overlayOpen })
    entered.current = mini
    if (action === 'leave') setMiniMode(false)
    else if (action === 'close-overlays') closeOverlays()
  }, [mini, blocking, overlayOpen, closeOverlays])
}
