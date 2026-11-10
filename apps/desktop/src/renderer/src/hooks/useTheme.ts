import { ConfigKey } from '@mika/sdk/contracts'
import { t } from '@mika/sdk/i18n'
import { toDisplayError } from '@mika/sdk-ui'
import { useCallback, useEffect, useRef, useState } from 'react'
import { applyTheme } from '../lib/apply-theme'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'
import { useUIStore } from '../store/useUIStore'
import { DEFAULT_THEME_ID, getTheme } from '../themes'

export function useTheme() {
  const [themeId, setThemeId] = useState(DEFAULT_THEME_ID)
  const receivedLiveUpdate = useRef(false)

  useEffect(() => {
    let eAPI: ReturnType<typeof getElectronApi>
    try {
      eAPI = getElectronApi()
    } catch (err) {
      rendererLog.warn('[useTheme] electronAPI unavailable; using default theme', err)
      return
    }

    const loadTheme = async () => {
      try {
        // Rota estreita: em janela de plugin `getSettingsConfig` é negado e o tema caía no padrão.
        const savedTheme = (await eAPI.runtime.getUiPrefs()).theme
        if (!receivedLiveUpdate.current && typeof savedTheme === 'string') {
          setThemeId(savedTheme)
        }
      } catch (err) {
        const message = toDisplayError(err, String(err))
        if (message.includes('denied by policy')) {
          rendererLog.debug('[useTheme] janela sem acesso a settings; usando o tema padrão')
          return
        }
        rendererLog.error('[useTheme] Failed to load theme:', err)
      }
    }
    void loadTheme()

    const unsub = eAPI.runtime.onConfigChanged((change) => {
      if (change.path === ConfigKey.CharacterTheme) {
        receivedLiveUpdate.current = true
        setThemeId(change.value as string)
      }
    })
    return unsub
  }, [])

  const currentTheme = getTheme(themeId)

  useEffect(() => {
    applyTheme(currentTheme)
  }, [currentTheme])

  const setTheme = useCallback((id: string) => {
    setThemeId((previousId) => {
      const run = async () => {
        try {
          const eAPI = getElectronApi()
          await eAPI.runtime.saveSetting(ConfigKey.CharacterTheme, id)
        } catch (err) {
          rendererLog.error('[useTheme] Failed to save theme:', err)
          setThemeId(previousId)
          useUIStore.getState().addNotification('error', t('shell.theme.saveFailed'), t('shell.theme.title'))
        }
      }
      void run()
      return id
    })
  }, [])

  return { currentTheme, setTheme }
}
