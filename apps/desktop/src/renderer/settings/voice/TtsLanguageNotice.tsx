import { GlassCard } from '@mika/sdk-ui/settings'
import { TriangleAlert } from 'lucide-react'
import type { Translate } from '../../src/hooks/useI18n'
import { languageDisplayName, ttsCoverageNotice } from './tts-language-coverage'
import { PROVIDERS, type TTSProvider } from './tts-provider-constants'

/**
 * The active local voice provider cannot speak the interface language: say so, and say what happens
 * (Piper fallback) or what to do (cloud provider) — silence with no explanation reads as a broken app.
 */
export function TtsLanguageNotice({
  t,
  locale,
  provider,
  piperVoice,
}: {
  t: Translate
  locale: string
  provider: TTSProvider
  piperVoice: string
}) {
  const key = ttsCoverageNotice(provider, locale, piperVoice)
  if (!key) return null
  const providerName = PROVIDERS.find((p) => p.id === provider)?.name ?? provider
  return (
    <GlassCard span="full">
      <div role="alert" className="flex items-start gap-2.5 rounded-xl border border-warn-rim bg-warn-soft p-3">
        <TriangleAlert size={15} className="mt-0.5 shrink-0 text-warn" aria-hidden="true" />
        <div className="min-w-0">
          <p className="text-caption font-black uppercase tracking-[0.12em] text-warn">
            {t('settings.voiceTts.coverage.title')}
          </p>
          <p className="mt-0.5 text-caption leading-snug text-text-main">
            {t(key, { provider: providerName, language: languageDisplayName(locale) })}
          </p>
        </div>
      </div>
    </GlassCard>
  )
}
