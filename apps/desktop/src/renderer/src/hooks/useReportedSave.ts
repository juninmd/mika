import { createUiLogger, toDisplayError } from '@mika/sdk-ui'
import { useCallback } from 'react'
import { useSaveStateStore } from './useSaveState'

const log = createUiLogger('settings:save')

// Per-path call sequence, shared across hook instances, so only the latest call's result updates the save status (guards against out-of-order resolution).
const latestSeqByPath = new Map<string, number>()

export function useReportedSave(
  onSave: (path: string, value: unknown) => Promise<void>,
): (path: string, value: unknown) => Promise<void> {
  const markSaving = useSaveStateStore((s) => s.markSaving)
  const markSaved = useSaveStateStore((s) => s.markSaved)
  const markError = useSaveStateStore((s) => s.markError)

  return useCallback(
    async (path: string, value: unknown) => {
      const seq = (latestSeqByPath.get(path) ?? 0) + 1
      latestSeqByPath.set(path, seq)
      markSaving(path)
      try {
        await onSave(path, value)
        if (latestSeqByPath.get(path) === seq) markSaved(path)
      } catch (error) {
        const message = toDisplayError(error, String(error))
        log.error('não consegui salvar a configuração', { configPath: path, error: message })
        if (latestSeqByPath.get(path) === seq) markError(path, message)
      }
    },
    [onSave, markSaving, markSaved, markError],
  )
}
