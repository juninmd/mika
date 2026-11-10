import { Wind } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { GlassSection, GlassSlider, GlassToggle } from './ds'
import { configBool, configNum } from './settings-utils'

type Props = {
  config: Record<string, unknown>
  save: (path: string, value: unknown) => Promise<void>
}

/** Wind in the avatar's hair, skirt and petals (`useAvatarWind` reads it in both render windows). */
export function SkinWindSection({ config, save }: Props) {
  const { t } = useI18n()
  const enabled = configBool(config, 'character.windEnabled', true)
  return (
    <GlassSection title={t('settings.skin.wind.title')} icon={Wind} description={t('settings.skin.wind.description')}>
      <div className="grid gap-4 md:grid-cols-2">
        <GlassToggle
          label={t('settings.character.field.windEnabled.label')}
          description={t('settings.character.field.windEnabled.description')}
          configPath="character.windEnabled"
          value={enabled}
          onSave={save}
        />
        <GlassSlider
          label={t('settings.character.field.windStrength.label')}
          description={t('settings.character.field.windStrength.description')}
          configPath="character.windStrength"
          config={config}
          value={configNum(config, 'character.windStrength', 35)}
          min={0}
          max={100}
          step={5}
          minLabel={t('settings.skin.wind.calm')}
          maxLabel={t('settings.skin.wind.storm')}
          disabled={!enabled}
          onSave={save}
        />
      </div>
    </GlassSection>
  )
}
