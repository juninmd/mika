import { choiceOption } from '@mika/sdk-ui/settings'
import { BellRing, CircleOff, DownloadCloud, RefreshCcwDot } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { GlassSection, GlassSelect } from './ds'

type Props = {
  config: Record<string, unknown>
  save: (path: string, value: unknown) => Promise<void>
}

/**
 * `app.autoUpdate` — lido pelo laço de fundo do backend (Rust `modules/update.rs` → `update/auto.rs`):
 * só avisa, ou baixa e instala sozinha quando o PC fica ocioso. Não aparece na build Steam, onde quem
 * atualiza é o cliente Steam.
 */
export function AutoUpdateSection({ config, save }: Props) {
  const { t } = useI18n()
  return (
    <GlassSection
      title={t('settings.update.auto.title')}
      icon={RefreshCcwDot}
      description={t('settings.update.auto.description')}
    >
      <div className="max-w-2xl">
        <GlassSelect
          label={t('settings.update.auto.label')}
          configPath="app.autoUpdate"
          config={config}
          options={[
            choiceOption(
              t('settings.update.auto.option.install'),
              'install',
              t('settings.update.auto.hint.install'),
              DownloadCloud,
            ),
            choiceOption(
              t('settings.update.auto.option.notify'),
              'notify',
              t('settings.update.auto.hint.notify'),
              BellRing,
            ),
            choiceOption(t('settings.update.auto.option.off'), 'off', t('settings.update.auto.hint.off'), CircleOff),
          ]}
          onSave={save}
        />
      </div>
    </GlassSection>
  )
}
