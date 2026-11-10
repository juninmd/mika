import { toDisplayError } from '@mika/sdk-ui'
import { AuroraField, GlassCard, SectionHeader } from '@mika/sdk-ui/settings'
import { Loader2, Search } from 'lucide-react'
import { useState } from 'react'
import { useI18n } from '../../src/hooks/useI18n'
import { getElectronApi } from '../../src/lib/shell-api'

type Hit = { title: string; url: string; snippet: string }
type SearchResult = { success?: boolean; data?: { results?: Hit[] }; error?: string }
type Preview = { hits: Hit[]; latencyMs: number } | { error: string }

/** Runs the SAME `web` tool the model calls (`action: "search"`), with the engine saved above. */
export function BrowserSearchPreview() {
  const { t } = useI18n()
  const [query, setQuery] = useState('')
  const [running, setRunning] = useState(false)
  const [preview, setPreview] = useState<Preview | null>(null)

  const run = async () => {
    const q = query.trim()
    if (!q) return
    setRunning(true)
    const started = performance.now()
    try {
      const result = (await getElectronApi().runtime.executeTool('web', {
        action: 'search',
        query: q,
        limit: 3,
      })) as SearchResult
      if (result.success === false) throw new Error(result.error ?? t('settings.browser.preview.error'))
      setPreview({ hits: result.data?.results ?? [], latencyMs: Math.round(performance.now() - started) })
    } catch (err) {
      setPreview({ error: toDisplayError(err, t('settings.browser.preview.error')) })
    } finally {
      setRunning(false)
    }
  }

  return (
    <GlassCard span="full" hover={false}>
      <SectionHeader
        title={t('settings.browser.preview.title')}
        description={t('settings.browser.preview.description')}
        className="mb-3"
      />
      <form
        className="grid items-end gap-3 sm:grid-cols-[1fr_auto]"
        onSubmit={(e) => {
          e.preventDefault()
          void run()
        }}
      >
        <AuroraField
          label={t('settings.browser.preview.queryLabel')}
          hideLabel
          type="search"
          value={query}
          onChange={setQuery}
          placeholder={t('settings.browser.preview.placeholder')}
        />
        <button
          type="submit"
          disabled={running || !query.trim()}
          className="inline-flex h-10 items-center justify-center gap-1.5 rounded-xl border border-accent-rim bg-accent-soft px-4 text-xs font-bold text-accent transition hover:bg-accent/20 active:scale-[0.98] disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          {running ? (
            <Loader2 size={14} className="animate-spin" aria-hidden="true" />
          ) : (
            <Search size={14} aria-hidden="true" />
          )}
          {t('settings.browser.preview.run')}
        </button>
      </form>
      {preview && 'error' in preview && (
        <p
          role="alert"
          className="mt-3 rounded-lg border border-danger-rim bg-danger-soft px-3 py-2 text-caption text-danger"
        >
          {preview.error}
        </p>
      )}
      {preview && 'hits' in preview && (
        <div className="mt-3 space-y-2" role="status">
          <p className="text-caption text-text-muted">
            {t('settings.browser.preview.summary')
              .replace('{{count}}', String(preview.hits.length))
              .replace('{{ms}}', String(preview.latencyMs))}
          </p>
          {preview.hits.map((hit) => (
            <div key={hit.url} className="rounded-xl border border-border-subtle bg-surface-raise/40 px-3 py-2">
              <p className="truncate text-xs font-bold text-text-main">{hit.title || hit.url}</p>
              <p className="truncate font-mono text-caption text-info">{hit.url}</p>
              {hit.snippet && <p className="mt-1 line-clamp-2 text-caption text-text-muted">{hit.snippet}</p>}
            </div>
          ))}
        </div>
      )}
    </GlassCard>
  )
}
