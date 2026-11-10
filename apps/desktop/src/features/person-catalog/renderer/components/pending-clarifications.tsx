import { toDisplayError } from '@mika/sdk-ui'
import { GlassSection } from '@mika/sdk-ui/settings'
import { CircleHelp, UserCheck, X } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../../renderer/src/lib/shell-api'
import { useUIStore } from '../../../../renderer/src/store/useUIStore'
import type { PendingClarification } from '../person-catalog.types'

type Rpc = { call<T>(method: string, args?: unknown): Promise<T> }

/**
 * "Which brother?" — relation-only mentions that fit more than one person. Mika asks in chat; this
 * lets the user answer here too. Picking a person files the waiting facts on them; dismiss drops them.
 */
export function PendingClarifications({ rpc, onSettled }: { rpc: Rpc; onSettled?: () => void }) {
  const { t } = useI18n()
  const [pending, setPending] = useState<PendingClarification[]>([])

  const load = useCallback(async () => {
    try {
      const res = await rpc.call<{ success: boolean; data?: PendingClarification[] }>('person_catalog_clarifications')
      setPending(res?.success ? (res.data ?? []) : [])
    } catch {
      setPending([])
    }
  }, [rpc])

  useEffect(() => {
    void load()
    const unsubscribe = getElectronApi().runtime.onPersonCatalogUpdated?.(() => void load())
    return () => unsubscribe?.()
  }, [load])

  const settle = async (id: string, personId?: string) => {
    try {
      const res = await rpc.call<{ success: boolean; error?: string }>('person_catalog_clarification_settle', {
        id,
        personId,
      })
      if (!res?.success)
        useUIStore.getState().addNotification('error', res?.error ?? t('settings.personCatalog.memory.errorGeneric'))
    } catch (err) {
      useUIStore
        .getState()
        .addNotification('error', toDisplayError(err, t('settings.personCatalog.memory.errorGeneric')))
    }
    await load()
    onSettled?.()
  }

  if (!pending.length) return null
  return (
    <GlassSection
      title={t('settings.personCatalog.clarify.title')}
      icon={CircleHelp}
      badge={String(pending.length)}
      description={t('settings.personCatalog.clarify.description')}
    >
      <ul className="space-y-3">
        {pending.map((item) => (
          <li key={item.id} className="rounded-xl border border-warn-rim bg-warn-soft p-3">
            <p className="text-sm font-semibold text-text-main">
              {t('settings.personCatalog.clarify.question').replace('{{label}}', item.label)}
            </p>
            {item.facts.length > 0 && (
              <p className="mt-1 text-caption text-text-muted">
                <span className="font-bold uppercase tracking-[0.12em] text-micro">
                  {t('settings.personCatalog.clarify.factsLabel')}
                </span>{' '}
                {item.facts.join(' · ')}
              </p>
            )}
            <div className="mt-2.5 flex flex-wrap gap-1.5">
              {item.candidates.map((candidate) => (
                <button
                  key={candidate.id}
                  type="button"
                  onClick={() => void settle(item.id, candidate.id)}
                  className="inline-flex items-center gap-1.5 rounded-lg border border-accent-rim bg-accent-soft px-2.5 py-1.5 text-caption font-bold text-accent-text transition hover:bg-accent/25 active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                >
                  <UserCheck size={13} aria-hidden="true" />
                  {candidate.name}
                </button>
              ))}
              <button
                type="button"
                onClick={() => void settle(item.id)}
                className="inline-flex items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-raise px-2.5 py-1.5 text-caption font-bold text-text-muted transition hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              >
                <X size={13} aria-hidden="true" />
                {t('settings.personCatalog.clarify.dismiss')}
              </button>
            </div>
          </li>
        ))}
      </ul>
    </GlassSection>
  )
}
