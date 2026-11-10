/** O que a janela expõe sobre tela cheia (`shell.window`); ausente em plugin-view e no navegador puro. */
export type FullscreenWindowApi = {
  isFullscreen?: () => Promise<boolean>
  onFullscreenChange?: (callback: (payload: { fullscreen: boolean }) => void) => () => void
}

/**
 * Estado real da janela (lido do backend, não da config) + cada mudança empurrada por `window:fullscreen`.
 * Um evento que chegou antes da leitura inicial terminar é mais novo que ela, então vence.
 */
export function subscribeFullscreen(api: FullscreenWindowApi, notify: (fullscreen: boolean) => void): () => void {
  let active = true
  let pushed = false
  const off = api.onFullscreenChange?.(({ fullscreen }) => {
    pushed = true
    if (active) notify(fullscreen)
  })
  void api
    .isFullscreen?.()
    .then((fullscreen) => {
      if (active && !pushed) notify(fullscreen)
    })
    .catch(() => {
      // Superfície sem permissão sobre a janela: os eventos acima ainda alimentam o estado.
    })
  return () => {
    active = false
    off?.()
  }
}
