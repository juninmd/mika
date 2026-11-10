import { toDisplayError } from '@mika/sdk-ui'
import { BentoGrid, GlassCard, GlassSlider, GlassToggle } from '@mika/sdk-ui/settings'
import { useCallback } from 'react'
import type { FeatureComponentProps } from '../../src/components/settings/feature-components'
import { configBool, configNum, configStrArray } from '../../src/components/settings/settings-utils'
import { TagList } from '../../src/components/settings/TagList'
import { useI18n } from '../../src/hooks/useI18n'
import { useUIStore } from '../../src/store/useUIStore'

type MemoryPanelProps = Pick<FeatureComponentProps, 'config' | 'onSave'>

export function MemoryRetrievalControls({ config, onSave }: MemoryPanelProps) {
  const { t } = useI18n()

  return (
    <>
      <BentoGrid cols={3}>
        <GlassCard>
          <GlassToggle
            label={t('settings.memory.config.embeddingEnabled.label')}
            description={t('settings.memory.config.embeddingEnabled.description')}
            configPath="brain.embeddingEnabled"
            value={configBool(config, 'brain.embeddingEnabled', true)}
            onSave={onSave}
          />
        </GlassCard>
        <GlassCard>
          <GlassToggle
            label={t('settings.memory.config.rerankingEnabled.label')}
            description={t('settings.memory.config.rerankingEnabled.description')}
            configPath="brain.rerankingEnabled"
            value={configBool(config, 'brain.rerankingEnabled', true)}
            onSave={onSave}
          />
        </GlassCard>
        <GlassCard>
          <GlassToggle
            label={t('settings.memory.config.emotionalTagging.label')}
            description={t('settings.memory.config.emotionalTagging.description')}
            configPath="brain.emotionalTagging"
            value={configBool(config, 'brain.emotionalTagging', true)}
            onSave={onSave}
          />
        </GlassCard>
      </BentoGrid>

      <BentoGrid cols={2}>
        <GlassCard>
          <GlassSlider
            label={t('settings.memory.config.recencyBias.label')}
            description={t('settings.memory.config.recencyBias.description')}
            configPath="brain.recencyBias"
            value={configNum(config, 'brain.recencyBias', 0.25)}
            min={0}
            max={1}
            step={0.05}
            showValue={(v) => v.toFixed(2)}
            onSave={onSave}
          />
        </GlassCard>
        <GlassCard>
          <GlassSlider
            label={t('settings.brain.field.ragTopK.label')}
            configPath="brain.ragTopK"
            value={configNum(config, 'brain.ragTopK', 5)}
            min={1}
            max={20}
            step={1}
            onSave={onSave}
          />
        </GlassCard>
      </BentoGrid>
      <BentoGrid cols={2}>
        <GlassCard>
          <GlassSlider
            label={t('settings.memory.config.minSimilarity.label')}
            description={t('settings.memory.config.minSimilarity.description')}
            configPath="brain.minSimilarity"
            value={configNum(config, 'brain.minSimilarity', 0.25)}
            min={0}
            max={1}
            step={0.05}
            showValue={(v) => v.toFixed(2)}
            onSave={onSave}
          />
        </GlassCard>
        <GlassCard>
          <GlassSlider
            label={t('settings.memory.field.episodicListLimit.label')}
            description={t('settings.memory.field.episodicListLimit.description')}
            configPath="memory.episodicListLimit"
            value={configNum(config, 'memory.episodicListLimit', 500)}
            min={1}
            max={5000}
            step={100}
            onSave={onSave}
          />
        </GlassCard>
      </BentoGrid>
      <BentoGrid cols={1}>
        <GlassCard>
          <GlassSlider
            label={t('settings.memory.config.lexicalRerankWeight.label')}
            description={t('settings.memory.config.lexicalRerankWeight.description')}
            configPath="brain.lexicalRerankWeight"
            value={configNum(config, 'brain.lexicalRerankWeight', 0.15)}
            min={0}
            max={0.5}
            step={0.05}
            showValue={(v) => v.toFixed(2)}
            onSave={onSave}
          />
        </GlassCard>
      </BentoGrid>
    </>
  )
}

export function MemoryAutonomyControls({ config, onSave }: MemoryPanelProps) {
  const { t } = useI18n()

  const saveAutoApproveTools = useCallback(
    async (tags: string[]) => {
      try {
        await onSave('brain.autoApproveTools', tags)
      } catch (e) {
        useUIStore.getState().addNotification('error', toDisplayError(e, t('sdkUi.saveError')))
      }
    },
    [onSave, t],
  )

  return (
    <>
      <BentoGrid cols={2}>
        <GlassCard>
          <GlassToggle
            label={t('settings.memory.config.approveRiskyTools.label')}
            description={t('settings.memory.config.approveRiskyTools.description')}
            configPath="brain.approveRiskyTools"
            value={configBool(config, 'brain.approveRiskyTools', true)}
            onSave={onSave}
          />
        </GlassCard>
      </BentoGrid>

      <GlassCard>
        <TagList
          label={t('settings.memory.config.autoApproveTools.label')}
          description={t('settings.memory.config.autoApproveTools.description')}
          tags={configStrArray(config, 'brain.autoApproveTools')}
          onChange={(tags) => void saveAutoApproveTools(tags)}
          placeholder={t('settings.memory.panel.autoApproveToolsPlaceholder')}
        />
      </GlassCard>
    </>
  )
}
