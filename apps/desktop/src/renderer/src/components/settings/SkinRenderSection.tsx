import { choiceOption } from '@mika/sdk-ui/settings'
import { Gauge, MonitorSmartphone, ScanEye, Sparkles } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { fixed } from '../../lib/format-number'
import { renderPixelRatio, renderSharpnessFromConfig, watchRenderPixelRatio } from '../../lib/render-pixel-ratio'
import { GlassSection, GlassSelect, StatTile } from './ds'
import { configStr } from './settings-utils'

type Props = {
  config: Record<string, unknown>
  save: (path: string, value: unknown) => Promise<void>
}

/** Nitidez do render do avatar (`useRenderSharpness` aplica nas duas janelas de render). */
export function SkinRenderSection({ config, save }: Props) {
  const { t } = useI18n()
  const sharpness = renderSharpnessFromConfig(configStr(config, 'character.renderSharpness'))
  // Densidade DESTE monitor (o da janela de configurações): relida quando ela muda de tela.
  const [monitorDpr, setMonitorDpr] = useState(() => window.devicePixelRatio || 1)
  useEffect(() => watchRenderPixelRatio(() => setMonitorDpr(window.devicePixelRatio || 1)), [])
  const density = renderPixelRatio(monitorDpr, sharpness)
  return (
    <GlassSection
      title={t('settings.skin.render.title')}
      icon={ScanEye}
      description={t('settings.skin.render.description')}
    >
      <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,14rem)] md:items-start">
        <GlassSelect
          label={t('settings.character.field.renderSharpness.label')}
          description={t('settings.character.field.renderSharpness.description')}
          configPath="character.renderSharpness"
          config={config}
          options={[
            choiceOption(
              t('settings.character.field.renderSharpness.option.auto'),
              'auto',
              t('settings.character.field.renderSharpness.hint.auto'),
              MonitorSmartphone,
            ),
            choiceOption(
              t('settings.character.field.renderSharpness.option.performance'),
              'performance',
              t('settings.character.field.renderSharpness.hint.performance'),
              Gauge,
            ),
            choiceOption(
              t('settings.character.field.renderSharpness.option.sharp'),
              'sharp',
              t('settings.character.field.renderSharpness.hint.sharp'),
              Sparkles,
            ),
          ]}
          onSave={save}
        />
        <StatTile
          label={t('settings.skin.render.density')}
          value={`${fixed(density, 1)}×`}
          hint={t('settings.skin.render.densityHint').replace('{{dpr}}', fixed(monitorDpr, 1))}
          icon={ScanEye}
          tone={sharpness === 'auto' ? 'neutral' : 'accent'}
        />
      </div>
    </GlassSection>
  )
}
