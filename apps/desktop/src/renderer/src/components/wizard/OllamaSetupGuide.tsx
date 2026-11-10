import { getLLMProviderEntry } from '@mika/sdk/llm/llm-catalog-data'
import { Download, ExternalLink, Loader2, RefreshCw, ServerOff } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { PullProgress } from '../../../settings/llm/ollama-model-download'
import { useLlmStatus, useOllamaPull } from '../../../settings/llm/use-llm-status'
import { openExternalLink } from '../../lib/shell-links'
import type { OllamaGuideCopy } from './wizard-copy'

const OLLAMA_DOWNLOAD_URL = 'https://ollama.com/download'
/** The catalog's local default: small enough for a first run on most GPUs/CPUs, and it calls tools. */
const FIRST_MODEL = getLLMProviderEntry('ollama').defaultModel

const BUTTON =
  'inline-flex items-center gap-1.5 rounded-lg border px-3 py-2 text-xs font-semibold disabled:cursor-not-allowed disabled:opacity-60'

type Props = { copy: OllamaGuideCopy; model: string; onInstalled: (model: string) => void }

/** Ollama selected in the wizard: say plainly whether it runs and has a model, and fix it here. */
export function OllamaSetupGuide({ copy, model, onInstalled }: Props) {
  const { status, loading, refresh } = useLlmStatus()
  const { state, start, cancel } = useOllamaPull(status?.pullingModel)
  const doneModel = state.phase === 'done' ? state.model : null
  const onInstalledRef = useRef(onInstalled)
  onInstalledRef.current = onInstalled
  const [openFailed, setOpenFailed] = useState(false)
  useEffect(() => {
    if (doneModel) onInstalledRef.current(doneModel)
  }, [doneModel])

  if (!status) return null
  const { ollama } = status
  const chatModels = ollama.installed.filter((m) => !m.embedding)

  if (!ollama.reachable) {
    return (
      <div role="alert" className="mt-3 rounded-xl border border-warn-rim bg-warn-soft p-3">
        <p className="flex items-center gap-2 text-sm font-semibold text-text-main">
          <ServerOff size={14} className="text-warn" aria-hidden="true" />
          {copy.offlineTitle}
        </p>
        <p className="mt-1 text-xs leading-5 text-text-muted">{copy.offlineBody}</p>
        <div className="mt-2 flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void openExternalLink(OLLAMA_DOWNLOAD_URL, copy.openError).then((ok) => setOpenFailed(!ok))}
            className={`${BUTTON} border-accent-rim bg-accent-soft text-accent hover:bg-accent/25`}
          >
            <ExternalLink size={11} aria-hidden="true" />
            {copy.install}
          </button>
          <button
            type="button"
            onClick={refresh}
            disabled={loading}
            className={`${BUTTON} border-border-subtle bg-surface-raise text-text-muted hover:text-text-main`}
          >
            {loading ? <Loader2 size={11} className="animate-spin" /> : <RefreshCw size={11} />}
            {copy.retry}
          </button>
        </div>
        {openFailed && <p className="mt-2 text-caption text-danger">{copy.openError}</p>}
      </div>
    )
  }

  if (chatModels.length > 0 && state.phase !== 'running') {
    return (
      <p className="mt-2 text-xs text-ok">
        {copy.ready.replace('{{version}}', ollama.version ?? '').replace('{{count}}', String(chatModels.length))}
      </p>
    )
  }

  const target = model.trim() || FIRST_MODEL
  return (
    <div className="mt-3 space-y-2 rounded-xl border border-border-subtle bg-surface-raise p-3">
      <p className="text-sm font-semibold text-text-main">{copy.noModelsTitle}</p>
      <p className="text-xs leading-5 text-text-muted">{copy.noModelsBody}</p>
      {state.phase === 'running' ? (
        <PullProgress
          state={state}
          onCancel={cancel}
          labels={{ downloading: copy.pulling, done: copy.pullDone, error: copy.pullError, cancel: copy.cancel }}
        />
      ) : (
        <button
          type="button"
          onClick={() => start(target)}
          className={`${BUTTON} border-accent-rim bg-accent-soft text-accent hover:bg-accent/25`}
        >
          <Download size={11} />
          {copy.pull.replace('{{model}}', target)}
        </button>
      )}
      {state.phase === 'error' && (
        <p className="text-xs text-danger">
          {copy.pullError.replace('{{model}}', state.model)}
          {state.message ? ` — ${state.message}` : ''}
        </p>
      )}
    </div>
  )
}
