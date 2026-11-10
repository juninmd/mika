import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassCard } from '@mika/sdk-ui/settings'
import { VisionNoWindowProbeNotice, type VisionPrivacy, VisionReadinessBanner } from './vision-readiness'
import { getAnalysisModes, getProviderChoices } from './vision-settings-components'

function getRecognitionKey(game: boolean, code: boolean): keyof TranslationDict {
  if (game && code) return 'settings.vision.hero.recognitionGamesAndCode'
  if (game) return 'settings.vision.hero.recognitionGames'
  if (code) return 'settings.vision.hero.recognitionCode'
  return 'settings.vision.hero.recognitionGeneral'
}

function optionLabel(options: Array<{ label: string; value: string }>, value: string): string {
  return options.find((option) => option.value === value)?.label ?? value
}

function VisionFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2">
      <p className="text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">{label}</p>
      <p className="mt-1 line-clamp-2 break-words text-sm font-semibold leading-snug text-text-main" title={value}>
        {value}
      </p>
    </div>
  )
}

type VisionSettingsHeroProps = {
  t: (key: keyof TranslationDict) => string
  provider: string
  analysisMode: string
  captureIntervalMs: number
  codeAwareness: boolean
  gameAwareness: boolean
  privacy: VisionPrivacy
  onSavePrivacy: (path: string, value: boolean) => Promise<void>
}
// Current setup at a glance; the page header already carries the title and on/off state.
export function VisionSettingsHero({
  t,
  provider,
  analysisMode,
  captureIntervalMs,
  codeAwareness,
  gameAwareness,
  privacy,
  onSavePrivacy,
}: VisionSettingsHeroProps) {
  return (
    <GlassCard span="full" hover={false}>
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
        <VisionFact
          label={t('settings.vision.hero.provider')}
          value={getProviderChoices(t).find((p) => p.id === provider)?.label ?? provider}
        />
        <VisionFact
          label={t('settings.vision.hero.analysisMode')}
          value={optionLabel(getAnalysisModes(t), analysisMode)}
        />
        <VisionFact label={t('settings.vision.hero.interval')} value={`${(captureIntervalMs / 1000).toFixed(0)}s`} />
        <VisionFact
          label={t('settings.vision.hero.recognition')}
          value={t(getRecognitionKey(gameAwareness, codeAwareness))}
        />
      </div>
      <VisionReadinessBanner t={t} privacy={privacy} onSave={onSavePrivacy} />
      <VisionNoWindowProbeNotice t={t} />
    </GlassCard>
  )
}
