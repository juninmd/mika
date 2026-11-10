import type { LegalDocumentId } from '@mika/sdk/contracts'
import { GlassToggle } from '@mika/sdk-ui/settings'
import { Scale } from 'lucide-react'
import { useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { useLegalDocument } from '../../hooks/useLegalDocument'
import { quitApp } from '../../lib/legal-acceptance'
import { rendererLog } from '../../lib/renderer-logger'
import { ErrorBanner, SecondaryButton } from './WizardPrimitives'

/** Recusar os termos = sair do app; se o Rust não encerrar, avisa em vez de deixar o botão mudo. */
export function LegalQuitButton({ disabled }: { disabled?: boolean }) {
  const { t } = useI18n()
  const [quitFailed, setQuitFailed] = useState(false)

  const quit = () => {
    setQuitFailed(false)
    quitApp().catch((error) => {
      rendererLog.error('legal consent: quit failed', { error })
      setQuitFailed(true)
    })
  }

  return (
    <>
      <div className="mt-3 flex justify-start">
        <SecondaryButton onClick={quit} disabled={disabled}>
          {t('legal.consent.quit')}
        </SecondaryButton>
      </div>
      {quitFailed && <ErrorBanner message={t('legal.consent.quitError')} />}
    </>
  )
}

/** Os dois documentos que se aceitam (os demais, em Sobre, são só consulta) — ids de `legal.rs`. */
const ACCEPTED_DOCS = ['eula', 'privacy'] as const satisfies readonly LegalDocumentId[]

type LegalConsentProps = {
  accepted: boolean
  onAcceptedChange: (accepted: boolean) => void
  /** "Tenho 18 anos ou mais": obrigatório junto com o aceite (o app é para adultos). */
  ageConfirmed: boolean
  onAgeConfirmedChange: (confirmed: boolean) => void
  disabled?: boolean
  /** Já havia um aceite, de um texto anterior: avisa que os documentos mudaram. */
  updated?: boolean
  /** O dono do documento também precisa reler quando o usuário pede "Tentar de novo". */
  onRetry?: () => void
}

/**
 * Leitura de EULA + política com o aceite obrigatório. Um componente só, usado pelo passo de
 * privacidade do wizard e pelo diálogo bloqueante de `LegalGate`: quem hospeda decide o que o aceite
 * destrava; recusar (Sair) encerra o app daqui mesmo.
 */
export function LegalConsent({
  accepted,
  onAcceptedChange,
  ageConfirmed,
  onAgeConfirmedChange,
  disabled,
  updated,
  onRetry,
}: LegalConsentProps) {
  const { t } = useI18n()
  const [doc, setDoc] = useState<(typeof ACCEPTED_DOCS)[number]>('eula')
  const [attempt, setAttempt] = useState(0)
  const state = useLegalDocument(doc, attempt)

  return (
    <section
      aria-labelledby="legal-consent-title"
      className="rounded-2xl border border-border-subtle bg-surface-raise p-3.5"
    >
      <div className="flex items-start gap-2.5">
        <span
          aria-hidden="true"
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent"
        >
          <Scale size={16} />
        </span>
        <div className="min-w-0">
          <h3 id="legal-consent-title" className="text-sm font-semibold text-text-main">
            {t('legal.consent.title')}
          </h3>
          <p className="mt-0.5 text-xs leading-5 text-text-muted">{t('legal.consent.description')}</p>
          {updated && <p className="mt-1 text-xs font-semibold text-warn">{t('legal.consent.updated')}</p>}
        </div>
      </div>

      <div className="mt-3 flex flex-wrap gap-2">
        {ACCEPTED_DOCS.map((id) => (
          <button
            key={id}
            type="button"
            aria-pressed={doc === id}
            onClick={() => setDoc(id)}
            className={`rounded-lg border px-3 py-1.5 text-caption font-bold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus active:scale-95 ${
              doc === id
                ? 'border-accent-rim bg-accent-soft text-accent-text'
                : 'border-border-subtle text-text-main hover:bg-surface-raise-strong'
            }`}
          >
            {t(id === 'eula' ? 'settings.about.doc.eula.title' : 'settings.about.doc.privacy.title')}
          </button>
        ))}
      </div>

      <div className="mt-2">
        {state.status === 'loading' && (
          <p role="status" className="text-caption text-text-muted">
            {t('status.loading')}
          </p>
        )}
        {state.status === 'error' && (
          <>
            <ErrorBanner message={t('settings.about.loadError')} />
            <SecondaryButton
              onClick={() => {
                setAttempt((n) => n + 1)
                onRetry?.()
              }}
            >
              {t('button.retry')}
            </SecondaryButton>
          </>
        )}
        {state.status === 'ready' && (
          <section
            // biome-ignore lint/a11y/noNoninteractiveTabindex: a scroll area must take focus so the keyboard can scroll a long legal text
            tabIndex={0}
            aria-label={t('settings.about.reader.aria')}
            className="max-h-44 overflow-auto rounded-xl border border-border-subtle bg-bg-deep p-3 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            <pre className="whitespace-pre-wrap break-words font-mono text-caption leading-relaxed text-text-main">
              {state.text}
            </pre>
          </section>
        )}
      </div>

      <div
        className={`mt-3 rounded-2xl border p-3 ${
          accepted ? 'border-accent-rim bg-accent-soft' : 'border-border-subtle bg-surface-raise'
        }`}
      >
        <GlassToggle
          label={t('legal.consent.accept')}
          value={accepted}
          disabled={disabled}
          onChange={onAcceptedChange}
        />
      </div>
      <div
        className={`mt-2 rounded-2xl border p-3 ${
          ageConfirmed ? 'border-accent-rim bg-accent-soft' : 'border-border-subtle bg-surface-raise'
        }`}
      >
        <GlassToggle
          label={t('legal.consent.age')}
          description={t('legal.consent.ageDescription')}
          value={ageConfirmed}
          disabled={disabled}
          onChange={onAgeConfirmedChange}
        />
      </div>
      {!(accepted && ageConfirmed) && (
        <p className="mt-1.5 text-caption text-text-muted">{t('legal.consent.required')}</p>
      )}
      <p className="mt-1.5 text-caption text-text-muted">{t('legal.consent.aiNotice')}</p>

      <LegalQuitButton disabled={disabled} />
    </section>
  )
}
