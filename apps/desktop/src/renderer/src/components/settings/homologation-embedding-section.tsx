import { toDisplayError } from '@mika/sdk-ui'
import { AuroraField } from '@mika/sdk-ui/settings'
import { AlertCircle, CheckCircle2, Cpu } from 'lucide-react'
import { useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { getElectronApi } from '../../lib/shell-api'
import { GlassCard, SectionLabel } from './ds'

export function HomologationEmbeddingSection() {
  const { t } = useI18n()
  const [embedText, setEmbedText] = useState(t('settings.homologation.embedding.sampleText'))
  const [embedLoading, setEmbedLoading] = useState(false)
  const [embedResult, setEmbedResult] = useState<{ ok: boolean; message: string; latency?: number } | null>(null)

  const testEmbedding = async () => {
    setEmbedLoading(true)
    setEmbedResult(null)
    const start = Date.now()
    try {
      const api = getElectronApi()
      const res = await api.runtime.testPlugin({
        pluginId: 'mika.embeddings',
        prompt: embedText,
      })
      const latency = Date.now() - start
      setEmbedResult({
        ok: res.ok,
        message:
          res.message ||
          (res.ok
            ? t('settings.homologation.embedding.successMessage')
            : t('settings.homologation.embedding.errorMessage')),
        latency,
      })
    } catch (err) {
      setEmbedResult({
        ok: false,
        message: `${t('settings.homologation.embedding.requestFailed')}: ${toDisplayError(err, String(err))}`,
        latency: Date.now() - start,
      })
    } finally {
      setEmbedLoading(false)
    }
  }

  return (
    <GlassCard className="flex h-full flex-col">
      <SectionLabel icon={Cpu} className="mb-4">
        {t('settings.homologation.embedding.title')}
      </SectionLabel>

      <div className="space-y-4 flex-1 flex flex-col justify-between">
        <div className="space-y-3">
          <p className="text-xs text-text-muted">{t('settings.homologation.embedding.description')}</p>
          <AuroraField
            label={t('settings.homologation.embedding.inputLabel')}
            type="textarea"
            rows={3}
            value={embedText}
            onChange={setEmbedText}
            placeholder={t('settings.homologation.embedding.inputPlaceholder')}
          />
        </div>

        <div className="mt-4 space-y-4">
          <button
            type="button"
            onClick={() => void testEmbedding()}
            disabled={embedLoading || !embedText.trim()}
            className="flex w-full items-center justify-center gap-2 rounded-xl border border-accent-rim bg-accent-soft py-2 text-xs font-bold text-accent-text transition-colors hover:bg-accent/25 active:scale-[0.98] disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            {embedLoading
              ? t('settings.homologation.embedding.generating')
              : t('settings.homologation.embedding.testButton')}
          </button>

          {embedResult && (
            <div
              role={embedResult.ok ? 'status' : 'alert'}
              className={`p-3 rounded-lg border text-xs ${
                embedResult.ok ? 'bg-ok-soft border-ok-rim text-ok' : 'bg-danger-soft border-danger-rim text-danger'
              }`}
            >
              <div className="font-bold flex items-center gap-1.5 mb-1">
                {embedResult.ok ? (
                  <CheckCircle2 size={12} aria-hidden="true" />
                ) : (
                  <AlertCircle size={12} aria-hidden="true" />
                )}
                {embedResult.ok
                  ? t('settings.homologation.embedding.validationSuccess')
                  : t('settings.homologation.embedding.validationFailure')}
                {embedResult.latency && (
                  <span className="text-caption font-mono opacity-60 ml-auto">{embedResult.latency}ms</span>
                )}
              </div>
              <p className="opacity-80 text-caption leading-relaxed">{embedResult.message}</p>
            </div>
          )}
        </div>
      </div>
    </GlassCard>
  )
}
