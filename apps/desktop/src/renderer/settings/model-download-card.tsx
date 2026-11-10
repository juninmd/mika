import type { TranslationDict } from '@mika/sdk/i18n'
import { StatusPill } from '@mika/sdk-ui/settings'
import { Download, Loader2 } from 'lucide-react'
import { useState } from 'react'
import { getElectronApi } from '../src/lib/shell-api'

type State = { phase: 'idle' | 'running' | 'ready' } | { phase: 'error'; message: string }
type Key = keyof TranslationDict

type Props = {
  assetId: 'nemotron' | 'local-embedding' | 'vad' | 'whisper'
  labels: { hint: Key; ready: Key; download: Key; downloading: Key }
  t: (key: Key) => string
  /** Called after the backend confirmed the files, so a parent can refresh what it shows. */
  onReady?: () => void
}

// Same backend step as the wizard: verifies each pinned file and only downloads what is missing.
export function ModelDownloadCard({ assetId, labels, t, onReady }: Props) {
  const [state, setState] = useState<State>({ phase: 'idle' })
  const prepare = async () => {
    setState({ phase: 'running' })
    try {
      const res = await getElectronApi().runtime.prepareWizardAsset({ assetId })
      setState(res.success ? { phase: 'ready' } : { phase: 'error', message: res.message ?? '' })
      if (res.success) onReady?.()
    } catch (e) {
      setState({ phase: 'error', message: e instanceof Error ? e.message : String(e) })
    }
  }
  const running = state.phase === 'running'
  return (
    <div className="sm:col-span-2 flex flex-wrap items-center gap-2 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2 text-caption text-text-muted">
      <p className="min-w-[16rem] flex-1">{t(labels.hint)}</p>
      {state.phase === 'ready' && <StatusPill variant="ok" label={t(labels.ready)} />}
      {state.phase === 'error' && <StatusPill variant="error" title={state.message} />}
      <button
        type="button"
        onClick={() => void prepare()}
        disabled={running}
        className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-accent-rim bg-accent-soft px-3 py-2 text-xs font-semibold text-accent hover:bg-accent/25 disabled:cursor-not-allowed disabled:bg-surface-raise-strong disabled:text-text-muted"
      >
        {running ? <Loader2 size={11} className="animate-spin" /> : <Download size={11} />}
        {t(running ? labels.downloading : labels.download)}
      </button>
    </div>
  )
}
