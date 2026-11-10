import { toDisplayError } from '@mika/sdk-ui'
import { CheckCircle2, Eraser, ExternalLink, Loader2, PlayCircle, XCircle } from 'lucide-react'
import { useState } from 'react'
import { openExternalLink } from '../../lib/shell-links'
import { ConfirmButton } from './ds'
import type { CredentialField, TKey } from './provider-credential-specs'
import { type CredentialTestOutcome, runCredentialTest } from './provider-credential-tests'

type TestState = { status: 'idle' } | { status: 'testing' } | { status: 'done'; outcome: CredentialTestOutcome }

const ACTION_CLS =
  'inline-flex min-h-8 items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-raise px-2.5 py-1 text-caption font-semibold text-text-muted transition-colors hover:border-accent-rim hover:bg-accent-soft hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50'

const UNIT_KEY: Record<CredentialTestOutcome['unit'], TKey> = {
  models: 'settings.providerCredentials.test.models',
  voices: 'settings.providerCredentials.test.voices',
  apis: 'settings.providerCredentials.test.apis',
}

function outcomeText(outcome: CredentialTestOutcome, t: (key: TKey) => string): string {
  if (!outcome.ok) return outcome.error || t('settings.providerCredentials.test.failed')
  const parts = [t('settings.providerCredentials.test.ok')]
  if (typeof outcome.latencyMs === 'number') parts.push(`${outcome.latencyMs} ms`)
  if (typeof outcome.count === 'number')
    parts.push(t(UNIT_KEY[outcome.unit]).replace('{{count}}', String(outcome.count)))
  return parts.join(' · ')
}

function TestResultLine({ state, t }: { state: TestState; t: (key: TKey) => string }) {
  if (state.status !== 'done') return null
  const { outcome } = state
  const Icon = outcome.ok ? CheckCircle2 : XCircle
  return (
    <p
      role={outcome.ok ? 'status' : 'alert'}
      className={`flex items-start gap-1.5 text-caption leading-snug ${outcome.ok ? 'text-ok' : 'text-danger'}`}
    >
      <Icon size={13} className="mt-px shrink-0" aria-hidden="true" />
      <span className="min-w-0 break-words">{outcomeText(outcome, t)}</span>
    </p>
  )
}

/** "Testar" (real check), "Obter chave" (allowlisted link) and "Limpar" (confirm) for one credential. */
export function CredentialCardActions({
  field,
  config,
  filled,
  onSave,
  t,
}: {
  field: CredentialField
  config: Record<string, unknown>
  filled: boolean
  onSave: (path: string, value: unknown) => Promise<void>
  t: (key: TKey) => string
}) {
  const [state, setState] = useState<TestState>({ status: 'idle' })
  // Ollama falls back to localhost when the URL is empty, so it is testable unfilled.
  const canTest = field.test !== undefined && (filled || field.test === 'ollama')

  const test = async () => {
    if (!field.test) return
    setState({ status: 'testing' })
    try {
      setState({ status: 'done', outcome: await runCredentialTest(field.test, config) })
    } catch (err) {
      const error = toDisplayError(err, t('settings.providerCredentials.test.failed'))
      setState({ status: 'done', outcome: { ok: false, unit: 'models', error } })
    }
  }

  if (!field.test && !field.keyUrl && !filled) return null
  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-1.5">
        {field.test && (
          <button
            type="button"
            onClick={() => void test()}
            disabled={!canTest || state.status === 'testing'}
            title={canTest ? undefined : t('settings.providerCredentials.test.fillFirst')}
            aria-label={`${t('settings.providerCredentials.test.button')}: ${field.label}`}
            className={ACTION_CLS}
          >
            {state.status === 'testing' ? (
              <Loader2 size={12} className="animate-spin" aria-hidden="true" />
            ) : (
              <PlayCircle size={12} aria-hidden="true" />
            )}
            {t('settings.providerCredentials.test.button')}
          </button>
        )}
        {field.keyUrl && (
          <button
            type="button"
            onClick={() => void openExternalLink(field.keyUrl ?? '', t('settings.providerCredentials.getKeyError'))}
            aria-label={`${t('settings.providerCredentials.getKey')}: ${field.label}`}
            className={ACTION_CLS}
          >
            <ExternalLink size={12} aria-hidden="true" />
            {t('settings.providerCredentials.getKey')}
          </button>
        )}
        {filled && (
          <ConfirmButton
            icon={Eraser}
            label={t('settings.providerCredentials.clear')}
            confirmLabel={t('settings.providerCredentials.clearConfirm')}
            ariaLabel={`${t('settings.providerCredentials.clear')}: ${field.label}`}
            className="ml-auto"
            onConfirm={() => {
              setState({ status: 'idle' })
              void onSave(field.configPath, '')
            }}
          />
        )}
      </div>
      <TestResultLine state={state} t={t} />
    </div>
  )
}
