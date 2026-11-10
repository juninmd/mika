import { toDisplayError } from '@mika/sdk-ui'
import { useCallback } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { rendererLog } from '../../lib/renderer-logger'
import { useUIStore } from '../../store/useUIStore'

type SaveFn = (path: string, value: unknown) => Promise<void>

// Provider and model save in two steps; if the second fails, reverts the provider to keep the pair consistent.
export function useProviderModelTransaction(opts: {
  onSave: SaveFn
  providerPath: string
  modelPath: string
  currentProvider: string
  logTag: string
}) {
  const { t } = useI18n()
  const { onSave, providerPath, modelPath, currentProvider, logTag } = opts

  const notifySaveError = useCallback(
    (error: unknown) => {
      useUIStore.getState().addNotification('error', toDisplayError(error, t('settings.llm.panel.saveError')))
    },
    [t],
  )

  return useCallback(
    async (nextProviderId: string, nextModelValue: string) => {
      const previousProvider = currentProvider
      try {
        await onSave(providerPath, nextProviderId)
      } catch (error) {
        rendererLog.error(`[${logTag}] falha ao salvar provedor`, { error: String(error) })
        notifySaveError(error)
        return
      }
      try {
        await onSave(modelPath, nextModelValue)
      } catch (error) {
        rendererLog.error(`[${logTag}] falha ao salvar modelo, revertendo provedor`, { error: String(error) })
        try {
          await onSave(providerPath, previousProvider)
        } catch (revertError) {
          rendererLog.error(`[${logTag}] falha ao reverter provedor`, { error: String(revertError) })
        }
        notifySaveError(error)
      }
    },
    [onSave, providerPath, modelPath, currentProvider, notifySaveError, logTag],
  )
}
