import { BentoGrid, GlassCard, GlassSlider } from '@mika/sdk-ui/settings'
import type { FeatureComponentProps } from '../../src/components/settings/feature-components'
import { configNum } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'

type MemoryPanelProps = Pick<FeatureComponentProps, 'config' | 'onSave'>

// `memory.backend`/`memory.redisUrl` removidos: nenhum backend Redis existe no port Rust
// (`modules/memory/mod.rs` só abre o embarcado `rusqlite`) — o seletor prometia uma troca de
// backend que nunca teve efeito nenhum.
export function MemoryStorageControls({ config, onSave }: MemoryPanelProps) {
  const { t } = useI18n()
  return (
    <BentoGrid cols={2}>
      <GlassCard>
        <GlassSlider
          label={t('settings.memory.field.shortTermTtlHours.label')}
          description={t('settings.memory.field.shortTermTtlHours.description')}
          configPath="memory.shortTermTtlHours"
          value={configNum(config, 'memory.shortTermTtlHours', 168)}
          min={1}
          max={720}
          step={1}
          showValue={(v) => `${v}h`}
          onSave={onSave}
        />
      </GlassCard>
      <GlassCard>
        <GlassSlider
          label={t('settings.memory.field.longTermTtlDays.label')}
          description={t('settings.memory.field.longTermTtlDays.description')}
          configPath="memory.longTermTtlDays"
          value={configNum(config, 'memory.longTermTtlDays', 365)}
          min={1}
          max={1095}
          step={1}
          showValue={(v) => `${v}d`}
          onSave={onSave}
        />
      </GlassCard>
    </BentoGrid>
  )
}
