import { toDisplayError } from '@mika/sdk-ui'
import { GlassCard, SectionHeader } from '@mika/sdk-ui/settings'
import { ArrowUpRight, Brain, RefreshCw } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { navigateSettingsTab } from '../../../renderer/src/lib/settings-navigation'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'

type MemoryRow = { id: string; content: string; type: string; timestamp: number }
type ListMemoriesResult = { total: number; memories: MemoryRow[] }

const LIMIT = 20

export function PrivacyMemoryCard() {
  const { t, locale } = useI18n()
  const [rows, setRows] = useState<MemoryRow[] | null>(null)
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const result = await getElectronApi().runtime.executeTool<ListMemoriesResult>('list_memories', {
        limit: LIMIT,
      })
      if (result?.success === false) {
        setError(result.error ?? t('settings.privacy.panel.memoryError'))
        return
      }
      setRows(result?.data?.memories ?? [])
      setTotal(result?.data?.total ?? 0)
    } catch (err) {
      setError(toDisplayError(err, t('settings.privacy.panel.memoryError')))
    } finally {
      setLoading(false)
    }
  }, [t])

  useEffect(() => {
    void load()
  }, [load])

  return (
    <GlassCard span="full">
      <SectionHeader
        title={t('settings.privacy.panel.memoryTitle')}
        description={t('settings.privacy.panel.memoryDescription')}
        className="mb-4"
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => navigateSettingsTab('brain')}
              className="inline-flex shrink-0 items-center gap-1.5 rounded-xl border border-accent-rim bg-accent-soft px-3 py-2 text-xs font-bold text-accent transition hover:bg-accent/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <ArrowUpRight size={13} aria-hidden="true" />
              {t('settings.privacy.memory.openBrain')}
            </button>
            <button
              type="button"
              onClick={() => void load()}
              disabled={loading}
              aria-label={t('settings.privacy.panel.memoryRefresh')}
              className="inline-flex shrink-0 items-center gap-1.5 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2 text-xs font-bold text-text-main transition hover:border-accent-rim hover:bg-accent-soft hover:text-accent disabled:opacity-50"
            >
              <RefreshCw size={13} aria-hidden="true" className={loading ? 'animate-spin' : undefined} />
              {t('settings.privacy.panel.memoryRefresh')}
            </button>
          </div>
        }
      />

      {error && <p className="text-xs text-danger">{error}</p>}

      {!error && rows !== null && rows.length === 0 && (
        <p className="text-xs text-text-muted">{t('settings.privacy.panel.memoryEmpty')}</p>
      )}

      {!error && rows !== null && rows.length > 0 && (
        <>
          <ul className="max-h-80 space-y-2 overflow-y-auto [scrollbar-width:thin]">
            {rows.map((row) => (
              <li key={row.id} className="rounded-xl border border-border-subtle bg-surface-raise/40 p-3">
                <div className="flex items-center gap-2 text-micro font-bold uppercase tracking-[0.14em] text-text-muted">
                  <Brain size={11} aria-hidden="true" />
                  <span>{new Date(row.timestamp).toLocaleString(locale)}</span>
                </div>
                <p className="mt-1 break-words text-xs text-text-main [overflow-wrap:anywhere]">{row.content}</p>
              </li>
            ))}
          </ul>
          <p className="mt-3 text-micro uppercase tracking-[0.12em] text-text-muted">
            {t('settings.privacy.panel.memoryCount')
              .replace('{{shown}}', String(rows.length))
              .replace('{{total}}', String(total))}
          </p>
        </>
      )}
    </GlassCard>
  )
}
