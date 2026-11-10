import { StatusPill } from '@mika/sdk-ui/settings'
import { Cpu } from 'lucide-react'
import { useI18n } from '../../src/hooks/useI18n'
import { ModelDownloadCard } from '../model-download-card'
import { formatBytes, useLlmStatus } from './use-llm-status'

/** Local embeddings run one fixed model (e5, `inference::embedder`): there is nothing to pick, only
 * whether it is on disk. A model picker here listed the Ollama chat models instead. */
export function LocalEmbeddingCard() {
  const { t } = useI18n()
  const { status, refresh } = useLlmStatus()
  const local = status?.localEmbedding
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-3 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2.5">
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-accent-rim bg-accent-soft text-accent">
          <Cpu size={16} aria-hidden="true" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-semibold text-text-main">{local?.model ?? '—'}</span>
          <span className="block text-caption text-text-muted">
            {[
              t('settings.embeddings.local.dimensions').replace('{{count}}', String(local?.dimensions ?? 384)),
              local?.downloaded
                ? t('settings.embeddings.local.onDisk').replace('{{size}}', formatBytes(local.sizeBytes))
                : t('settings.embeddings.local.downloadSize'),
              t('settings.embeddings.local.offline'),
            ].join(' · ')}
          </span>
        </span>
        {local &&
          (local.downloaded ? (
            <StatusPill variant="ok" dot label={t('settings.embeddings.local.downloaded')} />
          ) : (
            <StatusPill variant="warn" dot label={t('settings.embeddings.local.missing')} />
          ))}
      </div>
      <ModelDownloadCard
        assetId="local-embedding"
        labels={{
          hint: 'settings.llm.panel.localEmbeddingHint',
          ready: 'settings.llm.panel.localEmbeddingReady',
          download: 'settings.llm.panel.localEmbeddingDownload',
          downloading: 'settings.llm.panel.localEmbeddingDownloading',
        }}
        t={t}
        onReady={refresh}
      />
    </div>
  )
}
