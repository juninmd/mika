import { useEffect } from 'react'
import { renderSharpnessFromConfig, setRenderSharpness } from '../lib/render-pixel-ratio'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'

/**
 * Mantém a nitidez do render (`character.renderSharpness`, controle na aba Skin) igual à config,
 * no boot e a cada mudança. Quem desenha reage por `watchRenderPixelRatio`, nos dois backends.
 */
export function useRenderSharpness(): void {
  useEffect(() => {
    let eAPI: ReturnType<typeof getElectronApi>
    try {
      eAPI = getElectronApi()
    } catch {
      return
    }
    let alive = true
    // Uma troca que chega antes da leitura inicial voltar é mais nova que ela: a leitura não a desfaz.
    let changedSinceMount = false
    eAPI.runtime
      .getSettingsConfig()
      .then(({ config }) => {
        const character = config.character as Record<string, unknown> | undefined
        if (alive && !changedSinceMount) setRenderSharpness(renderSharpnessFromConfig(character?.renderSharpness))
      })
      .catch((error: unknown) =>
        rendererLog.debug('[useRenderSharpness] config indisponível, nitidez automática', { error }),
      )
    const unsubscribe = eAPI.runtime.onConfigChanged((change) => {
      if (change.path !== 'character.renderSharpness') return
      changedSinceMount = true
      setRenderSharpness(renderSharpnessFromConfig(change.value))
    })
    return () => {
      alive = false
      unsubscribe?.()
    }
  }, [])
}
