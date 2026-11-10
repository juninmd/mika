import { AuroraField, GlassCard } from '@mika/sdk-ui/settings'
import { useState } from 'react'
import { useI18n } from '../../src/hooks/useI18n'
import { rendererLog } from '../../src/lib/renderer-logger'
import { getElectronApi } from '../../src/lib/shell-api'

type LlmTestResult = {
  ok: boolean
  message: string
}

export function LLMPromptValidationPanel() {
  const { t } = useI18n()
  const [prompt, setPrompt] = useState('')
  const [testing, setTesting] = useState(false)
  const [result, setResult] = useState<LlmTestResult | null>(null)

  const sendPrompt = async () => {
    if (!prompt.trim()) return
    setTesting(true)
    setResult(null)
    try {
      const api = getElectronApi()
      const res = await api.runtime.testPlugin({
        tabId: 'llm',
        pluginId: 'llm',
        prompt,
      })
      setResult({
        ok: res.ok,
        message:
          res.message ||
          (res.ok ? t('settings.llm.panel.testPromptSuccessDefault') : t('settings.llm.panel.testPromptErrorDefault')),
      })
    } catch (error) {
      rendererLog.error('llm prompt validation failed', error)
      setResult({
        ok: false,
        message: t('settings.llm.panel.testPromptCallError'),
      })
    } finally {
      setTesting(false)
    }
  }

  return (
    <GlassCard span="full">
      <div className="flex flex-col gap-3">
        <p className="text-xs leading-relaxed text-text-muted">{t('settings.llm.panel.testPromptDescription')}</p>
        <div className="flex items-end gap-2">
          <div className="flex-1">
            <AuroraField
              label={t('settings.llm.panel.testPromptHeader')}
              value={prompt}
              onChange={setPrompt}
              placeholder={t('settings.llm.panel.testPromptPlaceholder')}
            />
          </div>
          <button
            type="button"
            onClick={() => void sendPrompt()}
            disabled={testing || !prompt.trim()}
            className={`rounded-lg border px-4 py-2 text-xs font-bold transition-colors duration-200 disabled:cursor-not-allowed disabled:opacity-50 ${
              testing
                ? 'animate-pulse border-accent-rim bg-accent-soft text-accent-text'
                : 'border-accent-rim bg-accent-soft text-accent-text hover:bg-accent/25'
            }`}
          >
            {testing ? t('settings.llm.panel.testPromptSending') : t('settings.llm.panel.testPromptSubmit')}
          </button>
        </div>
        {result && <PromptResult result={result} />}
      </div>
    </GlassCard>
  )
}

function PromptResult({ result }: { result: LlmTestResult }) {
  const { t } = useI18n()
  return (
    <div
      className={`mt-2 rounded-lg border p-3 text-xs leading-relaxed ${
        result.ok ? 'border-ok-rim bg-ok-soft text-ok' : 'border-danger-rim bg-danger-soft text-danger'
      }`}
    >
      <div className="mb-1.5 flex items-center gap-1.5">
        <div
          className={`h-1.5 w-1.5 rounded-full ${
            result.ok
              ? 'bg-ok shadow-[0_0_8px_color-mix(in_srgb,var(--ok)_60%,transparent)]'
              : 'bg-danger shadow-[0_0_8px_color-mix(in_srgb,var(--danger)_60%,transparent)]'
          }`}
        />
        <span className="text-micro font-extrabold uppercase tracking-widest">
          {result.ok ? t('settings.llm.panel.testPromptResultOk') : t('settings.llm.panel.testPromptResultFail')}
        </span>
      </div>
      <p className="mt-1 whitespace-pre-wrap rounded-md border border-border-subtle bg-black/30 p-3 font-mono text-text-main">
        {result.message}
      </p>
    </div>
  )
}
