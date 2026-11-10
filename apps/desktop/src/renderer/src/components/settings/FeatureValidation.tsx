import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { AlertCircle, CheckCircle2, Cpu, Loader2, PlayCircle, Timer, X } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { getByPath } from './settings-utils'
import type { FeatureValidationState, useFeatureValidation } from './useFeatureValidation'

type ValidationState = ReturnType<typeof useFeatureValidation>

const LLM_MODEL_PATH: Record<string, string> = {
  google: 'llm.googleModel',
  openrouter: 'llm.openrouterModel',
  openaiCompatible: 'llm.openaiCompatibleModel',
  unsloth: 'llm.unslothModel',
  ollama: 'llm.ollamaModel',
}

/** The model the tab's check actually exercised, when the tab has one. */
export function testedModel(tabId: string, config: Record<string, unknown>): string | null {
  const read = (path: string) => {
    const value = getByPath(config, path)
    return typeof value === 'string' && value.trim() ? value.trim() : null
  }
  if (tabId === 'llm') {
    const provider = read('llm.provider') ?? 'ollama'
    return read(LLM_MODEL_PATH[provider] ?? 'llm.model') ?? read('llm.model')
  }
  if (tabId === 'vision') return read('vision.model')
  if (tabId === 'embeddings') return read('llm.embeddingModel')
  return null
}

export function FeatureValidationButton({
  feature,
  validation,
}: {
  feature: FeatureSettingsDeclaration
  validation: ValidationState
}) {
  const { t } = useI18n()
  const { running, scenario, validate } = validation
  return (
    <button
      type="button"
      onClick={validate}
      disabled={running}
      title={t(scenario.descriptionKey)}
      aria-label={t('settings.tabHero.test.ariaLabel').replace('{{label}}', feature.tabLabel)}
      className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-accent-rim bg-accent-soft px-3 text-xs font-semibold text-accent transition-colors hover:bg-accent/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-wait disabled:opacity-60"
    >
      {running ? (
        <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
      ) : (
        <PlayCircle className="size-3.5" aria-hidden="true" />
      )}
      {running ? t('settings.tabHero.test.running') : t('settings.tabHero.test.button')}
    </button>
  )
}

function Fact({ icon: Icon, label, value }: { icon: typeof Timer; label: string; value: string }) {
  return (
    <span className="inline-flex min-w-0 items-center gap-1 rounded-md border border-border-subtle bg-surface-raise px-2 py-0.5 text-caption text-text-muted">
      <Icon className="size-3 shrink-0" aria-hidden="true" />
      <span className="font-semibold uppercase tracking-[0.08em]">{label}</span>
      <span className="min-w-0 truncate font-mono text-text-main">{value}</span>
    </span>
  )
}

const RESULT_LOOK = {
  running: {
    box: 'border-border-subtle bg-surface-raise/60',
    text: 'text-text-muted',
    icon: Loader2,
    titleKey: 'settings.tabHero.test.running',
  },
  success: {
    box: 'border-ok-rim bg-ok-soft',
    text: 'text-ok',
    icon: CheckCircle2,
    titleKey: 'settings.tabHero.test.passed',
  },
  error: {
    box: 'border-danger-rim bg-danger-soft',
    text: 'text-danger',
    icon: AlertCircle,
    titleKey: 'settings.tabHero.test.failed',
  },
} as const

/** Inline result of the hero "Testar": outcome, latency, model and the check's own message. */
export function FeatureValidationResult({
  state,
  model,
  onDismiss,
}: {
  state: FeatureValidationState
  model: string | null
  onDismiss?: () => void
}) {
  const { t } = useI18n()
  if (state.status === 'idle') return null
  const look = RESULT_LOOK[state.status]
  const Icon = look.icon
  const running = state.status === 'running'
  return (
    <div role="status" aria-live="polite" className={`mt-4 rounded-xl border p-3 ${look.box}`}>
      <div className="flex flex-wrap items-center gap-2">
        <Icon className={`size-4 shrink-0 ${look.text} ${running ? 'animate-spin' : ''}`} aria-hidden="true" />
        <span className={`text-xs font-bold ${look.text}`}>{t(look.titleKey)}</span>
        {typeof state.latencyMs === 'number' && (
          <Fact icon={Timer} label={t('settings.tabHero.test.latency')} value={`${state.latencyMs} ms`} />
        )}
        {model && <Fact icon={Cpu} label={t('settings.tabHero.test.model')} value={model} />}
        {onDismiss && !running && (
          <button
            type="button"
            onClick={onDismiss}
            aria-label={t('settings.tabHero.test.dismiss')}
            className="ml-auto rounded-md p-1 text-text-muted transition hover:bg-surface-raise hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            <X className="size-3.5" aria-hidden="true" />
          </button>
        )}
      </div>
      <p className="mt-2 break-words text-xs leading-relaxed text-text-main [overflow-wrap:anywhere]">
        {state.message}
      </p>
    </div>
  )
}
