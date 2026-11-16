import { useEffect, useRef, useState } from 'react'
import { type MiniModeStyle, miniModeAction, parseMiniModeStyle } from '../lib/mini-mode'
import { getElectronApi } from '../lib/shell-api'
import { subscribeWindowFlag } from '../lib/window-fullscreen'

export function setMiniMode(on: boolean): void {
  void getElectronApi().shell.window?.setMiniMode?.(on)
}

const STYLE_PATH = 'app.miniModeStyle'

export function setMiniModeStyle(style: MiniModeStyle): void {
  void getElectronApi().runtime.saveSetting(STYLE_PATH, style)
}

/** Estilo do modo mini (`app.miniModeStyle`): quadrado da cabeça ou dock de borda. */
export function useMiniModeStyle(): MiniModeStyle {
  const [style, setStyle] = useState<MiniModeStyle>('head')
  useEffect(() => {
    const runtime = getElectronApi()?.runtime
    if (!runtime) return
    runtime
      .getSettingsConfig()
      .then(({ config }) => setStyle(parseMiniModeStyle((config.app as { miniModeStyle?: unknown })?.miniModeStyle)))
      .catch(() => {})
    return runtime.onConfigChanged((change) => {
      if (change.path === STYLE_PATH) setStyle(parseMiniModeStyle(change.value))
    })
  }, [])
  return style
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
