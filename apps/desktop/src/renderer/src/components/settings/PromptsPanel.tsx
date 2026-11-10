import { AuroraField, BentoGrid, EmptyState, GlassSection, PageHero, StatusPill } from '@mika/sdk-ui/settings'
import {
  Brain,
  Eye,
  Gamepad2,
  Loader2,
  type LucideIcon,
  MessageSquareText,
  ScrollText,
  SearchX,
  Sparkles,
  Users2,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import type { Translate } from '../../hooks/useI18n'
import { useI18n } from '../../hooks/useI18n'
import { rendererLog } from '../../lib/renderer-logger'
import { getElectronApi } from '../../lib/shell-api'
import { PromptCard, type PromptSummary } from './PromptCard'

const NAMESPACE_ICON: Record<string, LucideIcon> = {
  brain: Brain,
  chat: MessageSquareText,
  memory: MessageSquareText,
  character: Sparkles,
  'gamer-companion': Gamepad2,
  'person-catalog': Users2,
  scheduler: MessageSquareText,
  vision: Eye,
}

const KNOWN_NAMESPACES = new Set(Object.keys(NAMESPACE_ICON))

function metaFor(namespace: string, t: Translate): { title: string; description: string; icon: LucideIcon } {
  const icon = NAMESPACE_ICON[namespace] ?? MessageSquareText
  if (!KNOWN_NAMESPACES.has(namespace)) return { title: namespace, description: '', icon }
  return {
    title: t(`settings.prompts.ns.${namespace}.title` as Parameters<Translate>[0]),
    description: t(`settings.prompts.ns.${namespace}.description` as Parameters<Translate>[0]),
    icon,
  }
}

function groupByNamespace(prompts: PromptSummary[]): Map<string, PromptSummary[]> {
  const map = new Map<string, PromptSummary[]>()
  for (const p of prompts) {
    const list = map.get(p.namespace) ?? []
    list.push(p)
    map.set(p.namespace, list)
  }
  return map
}

function matchesQuery(prompt: PromptSummary, query: string): boolean {
  if (!query) return true
  const q = query.toLowerCase()
  return (
    prompt.namespace.toLowerCase().includes(q) ||
    prompt.key.toLowerCase().includes(q) ||
    prompt.defaultText.toLowerCase().includes(q) ||
    (prompt.overrideText ?? '').toLowerCase().includes(q)
  )
}

export function PromptsPanel() {
  const { t } = useI18n()
  const electronApi = getElectronApi()
  const [prompts, setPrompts] = useState<PromptSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [search, setSearch] = useState('')

  const reload = useCallback(async () => {
    try {
      const result = await electronApi.runtime.getPrompts()
      setPrompts(result.prompts)
    } finally {
      setLoading(false)
    }
  }, [electronApi.runtime])

  useEffect(() => {
    reload().catch((error: unknown) => rendererLog.warn('[Prompts] failed to load prompts', { error: String(error) }))
  }, [reload])

  const handleSave = useCallback(
    async (namespace: string, key: string, text: string) => {
      const result = await electronApi.runtime.savePrompt({ namespace, key, text })
      if (!result.ok) throw new Error(result.error)
      await reload()
    },
    [electronApi.runtime, reload],
  )

  const handleReset = useCallback(
    async (namespace: string, key: string) => {
      const result = await electronApi.runtime.resetPrompt({ namespace, key })
      if (!result.ok) throw new Error(result.error)
      await reload()
    },
    [electronApi.runtime, reload],
  )

  const filtered = useMemo(() => prompts.filter((p) => matchesQuery(p, search)), [prompts, search])
  const grouped = useMemo(() => groupByNamespace(filtered), [filtered])
  const customizedCount = useMemo(() => prompts.filter((p) => p.overrideText !== null).length, [prompts])

  if (loading) {
    return <EmptyState icon={Loader2} title={t('settings.prompts.loading')} size="block" />
  }

  const totalLabel = `${t('settings.prompts.statTotal')}: ${prompts.length}`
  const customizedLabel = `${t('settings.prompts.statCustomized')}: ${customizedCount}`
  return (
    <div className="space-y-6">
      <PageHero
        icon={ScrollText}
        title={t('settings.prompts.title')}
        description={t('settings.prompts.description')}
        note={t('settings.prompts.additionalNote')}
        pills={
          <>
            {/* role=status keeps the counts announced (and addressable) now that they are pills, not tiles. */}
            <span role="status" aria-label={totalLabel}>
              <StatusPill variant="neutral" label={totalLabel} />
            </span>
            <span role="status" aria-label={customizedLabel}>
              <StatusPill variant={customizedCount > 0 ? 'info' : 'neutral'} label={customizedLabel} dot />
            </span>
          </>
        }
      />

      <AuroraField
        label={t('settings.prompts.searchPlaceholder')}
        hideLabel
        type="search"
        value={search}
        onChange={setSearch}
        placeholder={t('settings.prompts.searchPlaceholder')}
      />

      {grouped.size === 0 && (
        <EmptyState
          icon={SearchX}
          title={t('settings.prompts.emptyTitle')}
          hint={t('settings.prompts.emptyHint')}
          size="block"
          className="rounded-2xl border border-dashed border-border-subtle"
        />
      )}

      <BentoGrid cols={1}>
        {[...grouped.entries()].map(([namespace, list]) => {
          const meta = metaFor(namespace, t)
          return (
            <GlassSection
              key={namespace}
              title={meta.title}
              icon={meta.icon}
              badge={String(list.length)}
              description={meta.description}
              collapsible
            >
              <BentoGrid cols="auto">
                {list.map((prompt) => (
                  <PromptCard
                    key={`${prompt.namespace}/${prompt.key}`}
                    prompt={prompt}
                    onSave={handleSave}
                    onReset={handleReset}
                  />
                ))}
              </BentoGrid>
            </GlassSection>
          )
        })}
      </BentoGrid>
    </div>
  )
}
