import { BentoGrid, GlassCard } from '@mika/sdk-ui/settings'
import { Database, Download, Loader2 } from 'lucide-react'
import type { FeatureComponentProps } from '../../src/components/settings/feature-components'
import { useI18n } from '../../src/hooks/useI18n'
import { MemoryHero } from './memory-hero'
import { MemoryLayers } from './memory-layers'
import { EpisodicCard, ErrorNote, FactsCard, PeopleCard, ProfileCard, RelationsCard } from './memory-panel-cards'
import { MemoryConfigSection, PromptTraceSection } from './memory-panel-sections'
import { useMemoryConsole } from './use-memory-console'

type MemoryPanelProps = Pick<FeatureComponentProps, 'config' | 'onSave'>

/** Mirrors Rust `resolve_self_subject`: the user's own facts vs facts filed under someone else. */
const USER_SUBJECTS = new Set(['usuario', 'usuário', 'user', 'eu', 'o usuario', 'mim'])
const isUserSubject = (subject?: string) => USER_SUBJECTS.has((subject ?? 'usuário').trim().toLowerCase())

export function MemoryPanel({ config, onSave }: MemoryPanelProps) {
  const { t } = useI18n()
  const {
    loading,
    error,
    snapshot,
    episodic,
    searchQuery,
    setSearchQuery,
    searchedTerm,
    newFactText,
    setNewFactText,
    factFilter,
    setFactFilter,
    newName,
    setNewName,
    busyFact,
    busyName,
    busyExport,
    load,
    searchEpisodic,
    deleteEpisodic,
    addFact,
    deleteFact,
    saveName,
    forgetName,
    forgetPerson,
    deleteRelationship,
    exportJson,
    graph,
    people,
    relationships,
    facts,
    hasData,
  } = useMemoryConsole()

  return (
    <div className="space-y-4">
      <BentoGrid cols="auto">
        <MemoryHero
          config={config}
          onSave={onSave}
          counts={{
            episodic: episodic.length,
            facts: facts.length,
            people: people.length,
            relationships: relationships.length,
          }}
        />
      </BentoGrid>

      <MemoryLayers
        snapshot={snapshot}
        counts={{ episodic: episodic.length, userFacts: facts.filter((f) => isUserSubject(f.subject)).length }}
      />

      <BentoGrid cols="auto">
        <GlassCard span="full">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-center gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-accent-rim bg-accent-soft text-accent">
                <Database size={17} aria-hidden="true" />
              </div>
              <div>
                <h3 className="text-sm font-semibold text-text-main">{t('settings.memory.panel.dataTitle')}</h3>
                <p className="mt-0.5 text-xs text-text-muted">
                  {hasData
                    ? t('settings.memory.panel.recordsActive').replace(
                        '{{count}}',
                        String(episodic.length + people.length + relationships.length + facts.length),
                      )
                    : t('settings.memory.panel.recordsEmpty')}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={load}
                disabled={loading}
                className="inline-flex items-center gap-1.5 rounded-xl border border-border-subtle bg-surface-raise px-3 py-1.5 text-xs font-bold text-text-main transition hover:border-accent-rim hover:bg-accent-soft hover:text-accent disabled:opacity-50"
              >
                {loading && <Loader2 size={13} className="animate-spin" />}
                {loading ? t('status.loading') : t('settings.modelPicker.refresh')}
              </button>
              <button
                type="button"
                onClick={exportJson}
                disabled={!graph || busyExport}
                className="inline-flex items-center gap-1.5 rounded-xl border border-border-subtle bg-surface-raise px-3 py-1.5 text-xs font-bold text-text-main transition hover:border-accent-rim hover:bg-accent-soft hover:text-accent disabled:opacity-50"
              >
                <Download size={13} />
                {t('shell.history.exportJson')}
              </button>
            </div>
          </div>

          <div className="space-y-4">
            {error && <ErrorNote message={error} />}

            {loading && !snapshot && (
              <div className="flex items-center gap-2 text-xs text-text-muted py-2">
                <Loader2 size={14} className="animate-spin" />
                {t('settings.memory.panel.loadingMemories')}
              </div>
            )}

            <EpisodicCard
              memories={episodic}
              query={searchQuery}
              searchedTerm={searchedTerm}
              onQueryChange={setSearchQuery}
              onSearch={(q) => void searchEpisodic(q)}
              onDelete={(id) => void deleteEpisodic(id)}
              busy={loading}
            />

            <FactsCard
              facts={facts}
              filter={factFilter}
              onFilterChange={setFactFilter}
              newFact={newFactText}
              onNewFactChange={setNewFactText}
              onAdd={() => void addFact()}
              onDelete={(id) => void deleteFact(id)}
              busy={busyFact}
            />

            <div className="grid gap-4 sm:grid-cols-2">
              <PeopleCard
                people={people}
                onForget={(name) => void forgetPerson(name)}
                loading={loading}
                error={error}
              />
              <RelationsCard
                relationships={relationships}
                onDelete={(id) => void deleteRelationship(id)}
                loading={loading}
                error={error}
              />
            </div>

            <ProfileCard
              name={newName}
              onNameChange={setNewName}
              onSave={() => void saveName()}
              onForget={() => void forgetName()}
              busy={busyName}
            />
          </div>
        </GlassCard>
      </BentoGrid>

      <MemoryConfigSection config={config} onSave={onSave} />

      <PromptTraceSection />
    </div>
  )
}
