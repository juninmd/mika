/** O que a janela expõe sobre tela cheia (`shell.window`); ausente em plugin-view e no navegador puro. */
export type FullscreenWindowApi = {
  isFullscreen?: () => Promise<boolean>
  onFullscreenChange?: (callback: (payload: { fullscreen: boolean }) => void) => () => void
}

/**
 * Estado real da janela (lido do backend, não da config) + cada mudança empurrada pelo evento dela
 * (`window:fullscreen`, `window:mini-mode`). Um evento que chegou antes da leitura inicial terminar
 * é mais novo que ela, então vence.
 */
export function subscribeWindowFlag(
  read: () => Promise<boolean> | undefined,
  onChange: (push: (value: boolean) => void) => (() => void) | undefined,
  notify: (value: boolean) => void,
): () => void {
  let active = true
  let pushed = false
  const off = onChange((value) => {
    pushed = true
    if (active) notify(value)
  })
  void read()
    ?.then((value) => {
      if (active && !pushed) notify(value)
    })
    .catch(() => {
      // Superfície sem permissão sobre a janela: os eventos acima ainda alimentam o estado.
    })
  return () => {
    active = false
    off?.()
  }
}

export function subscribeFullscreen(api: FullscreenWindowApi, notify: (fullscreen: boolean) => void): () => void {
  return subscribeWindowFlag(
    () => api.isFullscreen?.(),
    (push) => api.onFullscreenChange?.(({ fullscreen }) => push(fullscreen)),
    notify,
  )
}
