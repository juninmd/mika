import { Wand2 } from 'lucide-react'
import { useI18n } from '../../../hooks/useI18n'
import { AVATAR_MODELS, resolveLive2DModelPath } from '../../live2d/model-path'
import { PageHero, StatusPill } from '../ds'
import { SkinRenderSection } from '../SkinRenderSection'
import { SkinWindSection } from '../SkinWindSection'
import { configStr } from '../settings-utils'
import { ModelStation } from './ModelStation'
import { StudioChangeBar } from './StudioChangeBar'
import { useChangeJournal } from './useChangeJournal'

type SkinPanelProps = {
  config: Record<string, unknown>
  saveSetting: (path: string, value: unknown) => Promise<void>
}

export function SkinPanel({ config, saveSetting }: SkinPanelProps) {
  const { t } = useI18n()
  const journal = useChangeJournal(config, saveSetting)

  const resolvedPath = resolveLive2DModelPath({
    modelName: configStr(config, 'character.modelName'),
    modelPath: configStr(config, 'character.modelPath'),
  })
  const activeModel = AVATAR_MODELS.find((m) => m.path === resolvedPath)
  const isVrm = resolvedPath.toLowerCase().endsWith('.vrm')

  return (
    <section aria-label={t('settings.skin.panel.ariaLabel')} className="space-y-6">
      <PageHero
        icon={Wand2}
        title={t('settings.skin.panel.title')}
        description={t('settings.skin.panel.description')}
        pills={
          <>
            <StatusPill variant="accent" label={isVrm ? 'VRM' : 'Live2D'} />
            <StatusPill
              variant={activeModel ? 'ok' : 'info'}
              label={activeModel?.label ?? t('settings.skin.panel.customModel')}
              dot
            />
            <StatusPill
              variant="neutral"
              label={t('settings.skin.panel.modelCount').replace('{{count}}', String(AVATAR_MODELS.length))}
            />
          </>
        }
        note={t('settings.skin.panel.heroNote')}
      />

      <ModelStation config={config} save={journal.trackedSave} />

      <SkinWindSection config={config} save={saveSetting} />

      <SkinRenderSection config={config} save={saveSetting} />

      <StudioChangeBar journal={journal} />
    </section>
  )
}
