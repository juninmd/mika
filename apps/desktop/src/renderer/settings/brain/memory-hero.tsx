import { GlassCard, GlassToggle, StatTile } from '@mika/sdk-ui/settings'
import { HeartHandshake, History, Lightbulb, type LucideIcon, Users } from 'lucide-react'
import type { FeatureComponentProps } from '../../src/components/settings/feature-components'
import { configBool } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'

type MemoryHeroProps = Pick<FeatureComponentProps, 'config' | 'onSave'> & {
  counts: { episodic: number; facts: number; people: number; relationships: number }
}

const count = (template: string, n: number) => template.replace('{{count}}', String(n))

function MemoryFact({ label, value, icon }: { label: string; value: string; icon: LucideIcon }) {
  return <StatTile label={label} value={value} icon={icon} className="min-h-0 min-w-0 p-3" />
}

// The page header already names the tab and its on/off state; this card keeps only what it adds.
export function MemoryHero({ config, onSave, counts }: MemoryHeroProps) {
  const { t } = useI18n()
  return (
    <GlassCard span="full" hover={false}>
      <div className="grid items-center gap-5 lg:grid-cols-[minmax(0,1.2fr)_minmax(260px,0.8fr)]">
        <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
          <MemoryFact
            icon={History}
            label={t('settings.memory.hero.episodic')}
            value={count(t('settings.memory.hero.episodicValue'), counts.episodic)}
          />
          <MemoryFact
            icon={Lightbulb}
            label={t('settings.memory.hero.facts')}
            value={count(t('settings.memory.hero.factsValue'), counts.facts)}
          />
          <MemoryFact
            icon={Users}
            label={t('settings.memory.hero.people')}
            value={count(t('settings.memory.hero.peopleValue'), counts.people)}
          />
          <MemoryFact
            icon={HeartHandshake}
            label={t('settings.memory.hero.relationships')}
            value={count(t('settings.memory.hero.relationshipsValue'), counts.relationships)}
          />
        </div>
        <GlassToggle
          label={t('settings.memory.field.enabled.label')}
          description={t('settings.memory.field.enabled.description')}
          configPath="memory.enabled"
          value={configBool(config, 'memory.enabled', true)}
          onSave={onSave}
        />
      </div>
    </GlassCard>
  )
}
