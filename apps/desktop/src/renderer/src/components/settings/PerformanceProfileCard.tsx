import type { PowerProfileState } from '@mika/sdk/contracts'
import type { TranslationDict } from '@mika/sdk/i18n'
import { Gauge } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { EFFECT_QUALITY_MAX_FPS } from '../../lib/effect-quality'
import { getPowerSnapshot, subscribePower } from '../../lib/power-profile'
import { GlassSection, GlassSelect, StatusPill } from './ds'
import { InferenceAccelerationControl } from './InferenceAccelerationControl'

type Key = keyof TranslationDict

/** `app.performanceProfile` — lido pelo governador de energia (Rust `modules/system/governor`). */
const PROFILE_PATH = 'app.performanceProfile'
const PROFILES: ReadonlyArray<{ value: PowerProfileState['profile']; label: Key; hint: Key }> = [
  { value: 'auto', label: 'settings.performance.profile.auto', hint: 'settings.performance.profile.autoHint' },
  { value: 'economy', label: 'settings.performance.profile.economy', hint: 'settings.performance.profile.economyHint' },
  { value: 'quality', label: 'settings.performance.profile.quality', hint: 'settings.performance.profile.qualityHint' },
]
const STATES: Record<PowerProfileState['state'], { pill: 'ok' | 'inactive' | 'accent' | 'warn'; label: Key }> = {
  active: { pill: 'ok', label: 'settings.performance.state.active' },
  idle: { pill: 'inactive', label: 'settings.performance.state.idle' },
  gaming: { pill: 'accent', label: 'settings.performance.state.gaming' },
  battery: { pill: 'warn', label: 'settings.performance.state.battery' },
}

type Props = {
  config: Record<string, unknown>
  onSave: (path: string, value: string) => Promise<void>
}

function LiveReadout({ live }: { live: PowerProfileState | null }) {
  const { t } = useI18n()
  if (!live) return <StatusPill variant="loading" />
  return (
    <>
      <StatusPill variant={STATES[live.state].pill} label={t(STATES[live.state].label)} dot />
      <StatusPill
        variant="neutral"
        label={`${t('settings.performance.fpsLabel')} ${EFFECT_QUALITY_MAX_FPS[live.ceiling]} fps`}
      />
      <StatusPill
        variant="neutral"
        label={`${t('settings.performance.cadenceLabel')} ×${live.backgroundScale}`}
        title={t('settings.performance.cadenceHint')}
      />
      <StatusPill variant="neutral" label={`${t('settings.performance.threadsLabel')} ${live.onnxThreads}`} />
    </>
  )
}

/** Perfil de desempenho + o que o governador está fazendo agora (atualiza por evento, sem polling). */
export function PerformanceProfileCard({ config, onSave }: Props) {
  const { t } = useI18n()
  const [live, setLive] = useState(() => getPowerSnapshot().profile)
  useEffect(() => subscribePower((power) => setLive(power.profile)), [])

  return (
    <GlassSection
      title={t('settings.performance.title')}
      icon={Gauge}
      description={t('settings.performance.description')}
    >
      <div className="space-y-4">
        <GlassSelect
          label={t('settings.performance.profileLabel')}
          configPath={PROFILE_PATH}
          config={config}
          onSave={onSave}
          options={PROFILES.map(({ value, label, hint }) => ({ value, label: t(label), description: t(hint) }))}
        />
        <div role="status" aria-live="polite" className="flex flex-wrap items-center gap-2">
          <span className="text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">
            {t('settings.performance.liveLabel')}
          </span>
          <LiveReadout live={live} />
        </div>
        <InferenceAccelerationControl config={config} onSave={onSave} live={live?.acceleration} />
      </div>
    </GlassSection>
  )
}
