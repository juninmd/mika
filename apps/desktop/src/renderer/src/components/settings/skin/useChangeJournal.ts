import { t } from '@mika/sdk/i18n'
import { useCallback, useRef, useState } from 'react'
import { getByPath } from '../settings-utils'

type ChangeEntry = {
  path: string
  before: unknown
  after: unknown
  at: number
}

export type ChangeJournal = {
  entries: ChangeEntry[]
  trackedSave: (path: string, value: unknown) => Promise<void>
  revert: (path: string) => Promise<void>
  revertAll: () => Promise<void>
  clear: () => void
}

export function useChangeJournal(
  config: Record<string, unknown>,
  saveSetting: (path: string, value: unknown) => Promise<void>,
): ChangeJournal {
  const [entries, setEntries] = useState<ChangeEntry[]>([])
  const configRef = useRef(config)
  configRef.current = config
  // Guard de sequência por path: só a chamada MAIS RECENTE pra uma chave atualiza a entrada do
  // diff. Sem isso, cliques rápidos em duas skins disparam trackedSave('character.modelPath', ...)
  // concorrentes e a entrada final reflete a ordem de RESOLUÇÃO das promises, não a dos cliques.
  const sequenceRef = useRef(new Map<string, number>())

  const trackedSave = useCallback(
    async (path: string, value: unknown) => {
      const before = getByPath(configRef.current, path)
      const requestId = (sequenceRef.current.get(path) ?? 0) + 1
      sequenceRef.current.set(path, requestId)
      await saveSetting(path, value)
      if (sequenceRef.current.get(path) !== requestId) return
      setEntries((prev) => {
        const existing = prev.find((entry) => entry.path === path)
        const baseline = existing ? existing.before : before
        const rest = prev.filter((entry) => entry.path !== path)
        if (Object.is(baseline, value)) return rest
        return [...rest, { path, before: baseline, after: value, at: Date.now() }]
      })
    },
    [saveSetting],
  )

  const revert = useCallback(
    async (path: string) => {
      const entry = entries.find((item) => item.path === path)
      if (!entry) return
      await saveSetting(path, entry.before)
      setEntries((prev) => prev.filter((item) => item.path !== path))
    },
    [entries, saveSetting],
  )

  const revertAll = useCallback(async () => {
    const results = await Promise.allSettled(
      entries.map((entry) => saveSetting(entry.path, entry.before).then(() => entry.path)),
    )
    const revertedPaths = new Set(
      results.filter((r): r is PromiseFulfilledResult<string> => r.status === 'fulfilled').map((r) => r.value),
    )
    setEntries((prev) => prev.filter((entry) => !revertedPaths.has(entry.path)))
    const failures = results.filter((r): r is PromiseRejectedResult => r.status === 'rejected')
    if (failures.length > 0) {
      throw new Error(
        t('settings.skin.studioChangeBar.revertPartialFailed', { count: entries.length, failed: failures.length }),
      )
    }
  }, [entries, saveSetting])

  const clear = useCallback(() => setEntries([]), [])

  return { entries, trackedSave, revert, revertAll, clear }
}
