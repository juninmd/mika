import type { TranslationDict } from '@mika/sdk/i18n'
import { AuroraField, GlassCard, SectionHeader, StatusPill } from '@mika/sdk-ui/settings'
import { Check, Download, Loader2, X } from 'lucide-react'
import { useState } from 'react'
import type { LlmStatus } from '../../../shared/electron-api-system-types'
import { useI18n } from '../../src/hooks/useI18n'
import { formatBytes, type PullState, pullFraction, useOllamaPull } from './use-llm-status'

type Key = keyof TranslationDict

/**
 * Real names from the Ollama library; the size is the download of the default quantization, and
 * roughly the VRAM it needs before context. Checked on 2026-10-30 at ollama.com/library/<name>:
 * qwen3.5 (4b 3.3 GB, 9b 6.6 GB; tools + vision + thinking), gemma4 (12b 7.7 GB; vision + tools),
 * ministral-3 (8b 6.0 GB; vision + tools), embeddinggemma (622 MB, 768d, multilingual),
 * nomic-embed-text (274 MB, 768d).
 */
export const OLLAMA_SUGGESTIONS: { id: string; size: string; tag: Key }[] = [
  { id: 'qwen3.5:4b', size: '3.3 GB', tag: 'settings.llm.pull.tag.light' },
  { id: 'qwen3.5:9b', size: '6.6 GB', tag: 'settings.llm.pull.tag.balanced' },
  { id: 'ministral-3:8b', size: '6.0 GB', tag: 'settings.llm.pull.tag.balanced' },
  { id: 'gemma4:12b', size: '7.7 GB', tag: 'settings.llm.pull.tag.vision' },
  { id: 'embeddinggemma', size: '622 MB', tag: 'settings.llm.pull.tag.embedding' },
  { id: 'nomic-embed-text', size: '274 MB', tag: 'settings.llm.pull.tag.embedding' },
]

const BUTTON =
  'inline-flex shrink-0 items-center gap-1.5 rounded-lg border px-3 py-2 text-xs font-semibold disabled:cursor-not-allowed disabled:opacity-60'

/** Texts with `{{model}}`; the wizard has its own copy, settings uses the i18n dictionary. */
export type PullLabels = { downloading: string; done: string; error: string; cancel: string }

type PullProgressProps = { state: PullState; onCancel: () => void; labels: PullLabels }

export function PullProgress({ state, onCancel, labels }: PullProgressProps) {
  if (state.phase === 'idle') return null
  if (state.phase === 'done') {
    return <StatusPill variant="ok" dot label={labels.done.replace('{{model}}', state.model)} />
  }
  if (state.phase === 'error') {
    return (
      <p role="alert" className="text-caption text-danger">
        {labels.error.replace('{{model}}', state.model)}
        {state.message ? ` — ${state.message}` : ''}
      </p>
    )
  }
  const fraction = pullFraction(state)
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between gap-2 text-caption text-text-muted">
        <span className="truncate">
          {labels.downloading.replace('{{model}}', state.model)}
          {state.status ? ` · ${state.status}` : ''}
        </span>
        <span className="flex shrink-0 items-center gap-2">
          {fraction !== null && `${formatBytes(state.completed)} / ${formatBytes(state.total)}`}
          <button
            type="button"
            onClick={onCancel}
            className={`${BUTTON} border-border-subtle bg-surface-raise py-1 text-text-muted hover:text-text-main`}
          >
            <X size={11} />
            {labels.cancel}
          </button>
        </span>
      </div>
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
        className="h-1.5 overflow-hidden rounded-full bg-surface-raise-strong"
      >
        <div
          className={`h-full rounded-full bg-accent transition-[width] ${fraction === null ? 'w-1/3 animate-pulse' : ''}`}
          style={fraction === null ? undefined : { width: `${(fraction * 100).toFixed(1)}%` }}
        />
      </div>
    </div>
  )
}

type Props = {
  status: LlmStatus | null
  activeModel: string
  onUseModel: (model: string) => void
}

/** "Baixar modelo": any Ollama model by name, with curated suggestions and live progress. */
export function OllamaModelDownload({ status, activeModel, onUseModel }: Props) {
  const { t } = useI18n()
  const [name, setName] = useState('')
  const { state, start, cancel } = useOllamaPull(status?.pullingModel)
  const reachable = status?.ollama.reachable ?? false
  const installed = status?.ollama.installed ?? []
  const loaded = new Set((status?.ollama.loaded ?? []).map((m) => m.name))
  const isInstalled = (id: string) => installed.some((m) => m.name === id || m.name === `${id}:latest`)
  const running = state.phase === 'running'
  return (
    <GlassCard span="full">
      <SectionHeader
        title={t('settings.llm.pull.title')}
        description={t('settings.llm.pull.description')}
        className="mb-3"
      />
      {!reachable && <p className="mb-3 text-caption text-warn">{t('settings.llm.pull.offline')}</p>}
      <div className="grid gap-4 lg:grid-cols-2">
        <div className="space-y-3">
          <div className="flex items-end gap-2">
            <div className="min-w-0 flex-1">
              <AuroraField
                label={t('settings.llm.pull.fieldLabel')}
                value={name}
                onChange={setName}
                onSubmit={() => reachable && !running && start(name)}
                placeholder={t('settings.llm.pull.fieldPlaceholder')}
              />
            </div>
            <button
              type="button"
              onClick={() => start(name)}
              disabled={!reachable || running || !name.trim()}
              className={`${BUTTON} border-accent-rim bg-accent-soft text-accent hover:bg-accent/25`}
            >
              {running ? <Loader2 size={11} className="animate-spin" /> : <Download size={11} />}
              {t('settings.llm.pull.download')}
            </button>
          </div>
          <p className="text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
            {t('settings.llm.pull.suggestions')}
          </p>
          <div className="grid gap-2 sm:grid-cols-2">
            {OLLAMA_SUGGESTIONS.map((s) => (
              <button
                key={s.id}
                type="button"
                disabled={!reachable || running || isInstalled(s.id)}
                onClick={() => {
                  setName(s.id)
                  start(s.id)
                }}
                className="flex min-w-0 items-center justify-between gap-2 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2 text-left hover:border-accent-rim disabled:cursor-not-allowed disabled:opacity-60"
              >
                <span className="min-w-0">
                  <span className="block truncate text-sm font-semibold text-text-main">{s.id}</span>
                  <span className="block text-caption text-text-muted">
                    {t(s.tag)} · {s.size}
                  </span>
                </span>
                {isInstalled(s.id) ? (
                  <Check size={14} className="shrink-0 text-ok" />
                ) : (
                  <Download size={14} className="shrink-0 text-text-muted" />
                )}
              </button>
            ))}
          </div>
          <PullProgress
            state={state}
            onCancel={cancel}
            labels={{
              downloading: t('settings.llm.pull.downloading'),
              done: t('settings.llm.pull.done'),
              error: t('settings.llm.pull.error'),
              cancel: t('settings.llm.pull.cancel'),
            }}
          />
        </div>
        <div>
          <p className="mb-2 text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
            {t('settings.llm.pull.installedTitle')}
          </p>
          {installed.length === 0 ? (
            <p className="text-caption text-text-muted">{t('settings.llm.pull.installedEmpty')}</p>
          ) : (
            <ul className="divide-y divide-border-subtle rounded-lg border border-border-subtle bg-surface-raise">
              {installed.map((m) => (
                <li key={m.name} className="flex items-center gap-2 px-3 py-2">
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-semibold text-text-main">{m.name}</span>
                    <span className="block text-caption text-text-muted">
                      {[m.parameterSize, m.quantization, formatBytes(m.sizeBytes)].filter(Boolean).join(' · ')}
                    </span>
                  </span>
                  {loaded.has(m.name) && <StatusPill variant="accent" label={t('settings.llm.pull.inVram')} />}
                  {m.embedding ? (
                    <StatusPill variant="neutral" label={t('settings.llm.pull.tag.embedding')} />
                  ) : m.name === activeModel ? (
                    <StatusPill variant="ok" label={t('settings.llm.pull.active')} />
                  ) : (
                    <button
                      type="button"
                      onClick={() => onUseModel(m.name)}
                      className={`${BUTTON} border-border-subtle bg-glass py-1 text-text-muted hover:text-text-main`}
                    >
                      {t('settings.llm.pull.use')}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </GlassCard>
  )
}
