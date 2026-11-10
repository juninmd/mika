import { t } from '@mika/sdk/i18n'
import { toErrorMessage } from '@mika/sdk/utils/object'
import { toDisplayError } from '@mika/sdk-ui'
import { useCallback, useEffect, useState } from 'react'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'

type UpdateState = {
  checking: boolean
  available: boolean
  downloading: boolean
  downloaded: boolean
  installing: boolean
  progress: number
  version: string | null
  /** Changelog da versão encontrada, quando a release publicou um. */
  notes: string | null
  error: string | null

  checked: boolean
  /** Quem atualiza o binário; `null` até o health responder. Na Steam o cliente Steam atualiza. */
  channel: UpdateChannel | null
}

export type UpdateChannel = 'steam' | 'direct'

const initialState: UpdateState = {
  checking: false,
  available: false,
  downloading: false,
  downloaded: false,
  installing: false,
  progress: 0,
  version: null,
  notes: null,
  error: null,
  checked: false,
  channel: null,
}

// Health indisponível: comporta-se como a build direta, que é quem já checava ao montar.
async function resolveUpdateChannel(): Promise<UpdateChannel> {
  try {
    return (await getElectronApi().runtime.getHealth()).updateChannel
  } catch {
    return 'direct'
  }
}

export function useUpdateChecker() {
  const [state, setState] = useState<UpdateState>(initialState)

  const runUpdateAction = useCallback(
    async <T>(
      flag: 'checking' | 'downloading',
      action: () => Promise<T>,
      nextState: (result: T) => Partial<UpdateState>,
      fallbackError: string,
    ) => {
      setState((s) => ({ ...s, [flag]: true, error: null }))
      try {
        const result = await action()
        setState((s) => ({ ...s, [flag]: false, ...nextState(result) }))
      } catch (error) {
        setState((s) => ({ ...s, [flag]: false, error: toDisplayError(error, fallbackError) }))
      }
    },
    [],
  )

  const checkForUpdates = useCallback(
    () =>
      runUpdateAction(
        'checking',
        () => getElectronApi().runtime.checkForUpdates(),
        (result) => ({
          available: result.updateAvailable,
          version: result.version ?? null,
          notes: result.notes ?? null,
          error: result.error ?? null,
          checked: true,
        }),
        t('settings.update.error.check'),
      ),
    [runUpdateAction],
  )

  const downloadUpdate = useCallback(async () => {
    await runUpdateAction(
      'downloading',
      () => getElectronApi().runtime.downloadUpdate(),
      (result) => ({ error: result.error ?? null, downloaded: result.success && !result.error }),
      t('settings.update.error.download'),
    )
  }, [runUpdateAction])

  const installUpdate = useCallback(async () => {
    setState((s) => ({ ...s, installing: true, error: null }))
    try {
      const result = await getElectronApi().runtime.quitAndInstall()

      if (!result.success) {
        setState((s) => ({ ...s, installing: false, error: result.error ?? t('settings.update.error.install') }))
      }
    } catch (error) {
      setState((s) => ({ ...s, installing: false, error: toDisplayError(error, t('settings.update.error.install')) }))
    }
  }, [])

  const refreshStatus = useCallback(async () => {
    try {
      const api = getElectronApi()
      const status = await api.runtime.getUpdateStatus()
      setState((s) => ({
        ...s,
        available: status.available,
        version: status.version ?? s.version,
        progress: status.progress ?? s.progress,
        // O backend só reporta 100 com o pacote baixado e verificado — inclusive quando quem baixou
        // foi a atualização automática, sem ninguém clicar em nada nesta janela.
        downloaded: s.downloaded || status.progress === 100,
      }))
    } catch (error) {
      rendererLog.debug('[update] falha ao consultar status de atualização', { error: toErrorMessage(error) })
    }
  }, [])

  useEffect(() => {
    let mounted = true
    void (async () => {
      const channel = await resolveUpdateChannel()
      if (!mounted) return
      setState((s) => ({ ...s, channel }))
      if (channel !== 'direct') return
      // Pacote já baixado pela atualização automática: mostra "pronto para instalar" e NÃO checa de
      // novo — um check novo faz o backend descartar o pacote guardado.
      const status = await getElectronApi()
        .runtime.getUpdateStatus()
        .catch(() => null)
      if (!mounted) return
      if (status?.progress === 100) void refreshStatus()
      else void checkForUpdates()
    })()
    return () => {
      mounted = false
    }
  }, [checkForUpdates, refreshStatus])

  useEffect(() => {
    if (!state.downloading) return
    const timer = setInterval(() => void refreshStatus(), 1000)
    return () => clearInterval(timer)
  }, [state.downloading, refreshStatus])

  return {
    ...state,
    checkForUpdates,
    downloadUpdate,
    installUpdate,
    refreshStatus,
  }
}
