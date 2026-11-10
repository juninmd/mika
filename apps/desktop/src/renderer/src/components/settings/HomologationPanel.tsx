import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { Cpu, Loader2, PlayCircle, Stethoscope } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { getElectronApi } from '../../lib/shell-api'
import { BentoGrid, GlassSection, PageHero, StatTile, StatusPill } from './ds'
import { type BuildIdentity, HomologationBuildCard } from './HomologationBuildCard'
import { HomologationDevDetails } from './HomologationDevDetails'
import { FeatureRow, type FeatureTestResult } from './HomologationFeatureRow'
import { HomologationEmbeddingSection } from './homologation-embedding-section'
import { HomologationVadSection } from './homologation-vad-section'

type HomologationPanelProps = {
  declarations: FeatureSettingsDeclaration[]
  onSelectTab: (tabId: string) => void
}

/** Parallel lanes for "Rodar todos": enough to finish fast, few enough not to stampede one LLM. */
const RUN_ALL_LANES = 3

function useBuildIdentity(): BuildIdentity | null {
  const [build, setBuild] = useState<BuildIdentity | null>(null)
  useEffect(() => {
    let mounted = true
    getElectronApi()
      .runtime.getHealth()
      .then((health) => {
        if (!mounted) return
        const { version, packaged, status, uptime, features, tools } = health
        setBuild({ version, packaged, status, uptime, features, tools })
      })
      .catch(() => {})
    return () => {
      mounted = false
    }
  }, [])
  return build
}

async function testOne(tab: FeatureSettingsDeclaration, failure: string): Promise<FeatureTestResult> {
  const started = performance.now()
  const latencyMs = () => Math.round(performance.now() - started)
  try {
    const res = await getElectronApi().runtime.testPlugin({ pluginId: tab.pluginId || tab.tabId })
    return { ok: res.ok, message: res.message, latencyMs: latencyMs() }
  } catch {
    return { ok: false, message: failure, latencyMs: latencyMs() }
  }
}

export function HomologationPanel({ declarations, onSelectTab }: HomologationPanelProps) {
  const { t } = useI18n()
  const [testing, setTesting] = useState<Set<string>>(new Set())
  const [results, setResults] = useState<Record<string, FeatureTestResult>>({})
  const [runningAll, setRunningAll] = useState(false)
  const build = useBuildIdentity()

  const runTest = async (tab: FeatureSettingsDeclaration) => {
    setTesting((prev) => new Set(prev).add(tab.tabId))
    const result = await testOne(tab, t('settings.homologation.panel.rpcConnectionFailed'))
    setResults((prev) => ({ ...prev, [tab.tabId]: result }))
    setTesting((prev) => {
      const next = new Set(prev)
      next.delete(tab.tabId)
      return next
    })
  }

  const runAll = async () => {
    setRunningAll(true)
    setResults({})
    const queue = [...declarations]
    const lane = async () => {
      for (let tab = queue.shift(); tab; tab = queue.shift()) await runTest(tab)
    }
    await Promise.all(Array.from({ length: RUN_ALL_LANES }, lane))
    setRunningAll(false)
  }

  const done = Object.values(results)
  const passed = done.filter((r) => r.ok).length
  const failed = done.length - passed
  const total = declarations.length

  return (
    <div className="space-y-6">
      <PageHero
        icon={Stethoscope}
        title={t('settings.homologation.panel.title')}
        description={t('settings.homologation.panel.subtitle')}
        pills={
          done.length > 0 && (
            <StatusPill
              variant={runningAll ? 'info' : failed > 0 ? 'warn' : 'ok'}
              label={t('settings.diagnostics.summary')
                .replace('{{passed}}', String(passed))
                .replace('{{total}}', String(total))
                .replace('{{failed}}', String(failed))}
              dot
            />
          )
        }
        actions={
          <button
            type="button"
            onClick={() => void runAll()}
            disabled={runningAll}
            className="inline-flex h-9 items-center gap-2 rounded-lg border border-accent-rim bg-accent-soft px-4 text-xs font-bold text-accent transition hover:bg-accent/20 active:scale-[0.98] disabled:cursor-wait disabled:opacity-60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            {runningAll ? (
              <Loader2 size={14} className="animate-spin" aria-hidden="true" />
            ) : (
              <PlayCircle size={14} aria-hidden="true" />
            )}
            {runningAll
              ? t('settings.diagnostics.runningAll')
                  .replace('{{done}}', String(done.length))
                  .replace('{{total}}', String(total))
              : t('settings.diagnostics.runAll')}
          </button>
        }
        aside={
          <>
            {/* Untested is not failed: only a full pass earns the status colour. */}
            <StatTile
              label={t('settings.homologation.panel.testsOk')}
              value={`${passed}/${total}`}
              tone={total > 0 && passed === total ? 'ok' : 'neutral'}
            />
            <StatTile
              label={t('settings.diagnostics.failures')}
              value={String(failed)}
              tone={failed > 0 ? 'danger' : 'neutral'}
            />
          </>
        }
      />

      <BentoGrid cols="auto">
        <HomologationBuildCard build={build} />
      </BentoGrid>

      <GlassSection title={t('settings.homologation.panel.modulesStatus')} icon={Cpu} badge={String(total)}>
        <div className="overflow-hidden rounded-xl border border-border-subtle bg-surface-raise/20">
          <table
            className="w-full text-left text-xs border-collapse"
            aria-label={t('settings.homologation.panel.modulesTableAriaLabel')}
          >
            <thead>
              <tr className="border-b border-border-subtle bg-surface-raise">
                <th className="px-4 py-3 text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">
                  {t('settings.homologation.panel.columnModule')}
                </th>
                <th className="px-4 py-3 text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">
                  {t('settings.homologation.panel.columnStatus')}
                </th>
                <th className="px-4 py-3 text-caption font-semibold uppercase tracking-[0.12em] text-text-muted text-right">
                  {t('settings.homologation.panel.columnAction')}
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border-subtle">
              {declarations.map((tab) => (
                <FeatureRow
                  key={tab.tabId}
                  tab={tab}
                  testResult={results[tab.tabId]}
                  isTesting={testing.has(tab.tabId)}
                  onTest={() => void runTest(tab)}
                  onSelect={() => onSelectTab(tab.tabId)}
                />
              ))}
            </tbody>
          </table>
        </div>
      </GlassSection>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <HomologationEmbeddingSection />
        <HomologationVadSection onOpenStt={() => onSelectTab('stt')} />
      </div>

      {build && !build.packaged && <HomologationDevDetails declarations={declarations} />}
    </div>
  )
}
