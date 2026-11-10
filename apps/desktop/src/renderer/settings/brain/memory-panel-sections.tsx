import { GlassCard, GlassSection } from '@mika/sdk-ui/settings'
import { FileText, Loader2, Search, SlidersHorizontal } from 'lucide-react'
import { useState } from 'react'
import type { FeatureComponentProps } from '../../src/components/settings/feature-components'
import { useI18n } from '../../src/hooks/useI18n'
import { getElectronApi } from '../../src/lib/shell-api'
import { MemoryAutonomyControls, MemoryRetrievalControls } from './memory-config-controls'
import {
  type EpisodicMemory,
  ErrorNote,
  type LearnedFact,
  type PersonRecord,
  type RelationshipEdge,
} from './memory-panel-cards'
import { MemoryStorageControls } from './memory-storage-controls'

export type SnapshotData = {
  promptTrace?: {
    systemPrompt?: string
    estimatedTokens?: number
    context?: Array<{ provider: string; durationMs: number }>
  } | null
  episodic?: EpisodicMemory[]
  /** Short-term layer: raw recent window + rolling summary + facts established this conversation. */
  shortTerm?: { messages: number; rollingSummary?: string | null; sessionFacts: string[] }
  /** Long-term people layer (person catalog): people, facts on their cards, open "which one?" questions. */
  peopleMemory?: { people: number; facts: number; pendingClarifications: number } | null
  graph?: {
    profile?: { preferredName?: string } | null
    people?: PersonRecord[]
    relationships?: RelationshipEdge[]
    facts?: LearnedFact[]
  }
}

type MemoryPanelProps = Pick<FeatureComponentProps, 'config' | 'onSave'>

export function MemoryConfigSection({ config, onSave }: MemoryPanelProps) {
  const { t } = useI18n()

  return (
    <GlassSection title={t('settings.memory.panel.configTitle')} icon={SlidersHorizontal} collapsible defaultCollapsed>
      <div className="space-y-4">
        <MemoryRetrievalControls config={config} onSave={onSave} />
        <MemoryAutonomyControls config={config} onSave={onSave} />
        <MemoryStorageControls config={config} onSave={onSave} />
      </div>
    </GlassSection>
  )
}

export function PromptTraceSection() {
  const { t } = useI18n()
  const [trace, setTrace] = useState<SnapshotData['promptTrace']>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const loadTrace = async () => {
    setLoading(true)
    setError(null)
    try {
      const result = await getElectronApi().runtime.executeTool<SnapshotData>('memory_console', {
        action: 'snapshot',
      })
      if (!result.success) {
        setError(result.error ?? t('settings.memory.panel.loadPromptError'))
        return
      }
      setTrace(result.data?.promptTrace ?? null)
    } catch {
      setError(t('settings.memory.panel.loadPromptError'))
    } finally {
      setLoading(false)
    }
  }

  const providers = (trace?.context ?? []).map((c) => c.provider)
  const hasTrace = Boolean(trace?.systemPrompt)

  return (
    <GlassSection
      title={t('settings.memory.panel.lastPromptTitle')}
      icon={FileText}
      collapsible
      defaultCollapsed={true}
    >
      <div className="space-y-3">
        {error && <ErrorNote message={error} />}
        <button
          type="button"
          onClick={() => void loadTrace()}
          disabled={loading}
          className="inline-flex items-center gap-2 rounded-lg border border-border-subtle bg-surface-raise px-4 py-2 text-caption font-semibold text-text-main transition-colors hover:text-ok disabled:opacity-50"
        >
          {loading ? <Loader2 size={12} className="animate-spin" /> : <Search size={12} />}
          {t('settings.memory.panel.viewButton')}
        </button>
        {hasTrace && trace && (
          <GlassCard>
            <div className="mb-2 flex flex-wrap items-center gap-2 text-caption text-text-muted">
              <span className="font-mono text-ok">~{trace.estimatedTokens ?? '?'} tokens</span>
              {providers.length > 0 && (
                <span>
                  {t('settings.memory.panel.providersLabel')} {providers.join(', ')}
                </span>
              )}
            </div>
            <pre className="max-h-72 overflow-auto whitespace-pre-wrap rounded-lg border border-border-subtle bg-black/40 p-3 font-mono text-caption leading-relaxed text-text-muted">
              {trace.systemPrompt}
            </pre>
          </GlassCard>
        )}
        {!loading && !hasTrace && (
          <p className="text-caption text-text-subtle">{t('settings.memory.panel.noPromptYet')}</p>
        )}
      </div>
    </GlassSection>
  )
}
