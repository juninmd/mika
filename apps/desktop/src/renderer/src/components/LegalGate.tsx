import { motion } from 'framer-motion'
import { Scale } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useFocusTrap } from '../hooks/useFocusTrap'
import { useI18n } from '../hooks/useI18n'
import type { useLegalGate } from '../hooks/useLegalGate'
import { layerStyle } from '../lib/layer-styles'
import { saveLegalAcceptance } from '../lib/legal-acceptance'
import { rendererLog } from '../lib/renderer-logger'
import { LegalConsent, LegalQuitButton } from './wizard/LegalConsent'
import { ErrorBanner, PrimaryButton } from './wizard/WizardPrimitives'

/**
 * Aceite ao abrir o app quando a versão aceita não é a atual — usuário que já existia antes do
 * aceite, ou EULA/política atualizados. Bloqueante de propósito: sem Esc, sem clique no fundo, sem
 * fechar; as únicas saídas são aceitar ou Sair (que encerra o app, dentro de `LegalConsent`).
 * Primeira execução não passa por aqui: o wizard grava o aceite antes de revelar o avatar.
 * Se o texto não carrega, o app continua bloqueado (só Sair / Tentar de novo): sem texto não há
 * versão a aceitar, e abrir o app sem aceite seria falhar aberto. `App` marca o resto como `inert`
 * enquanto `open`, e aqui o foco entra no cartão ao abrir e volta a quem o tinha ao fechar.
 */
export function LegalGate({ gate }: { gate: ReturnType<typeof useLegalGate> }) {
  const { t } = useI18n()
  const { needed, failed, version, updated, retry } = gate
  const open = needed || failed
  const [accepted, setAccepted] = useState(false)
  const [ageConfirmed, setAgeConfirmed] = useState(false)
  const [saving, setSaving] = useState(false)
  const [saveFailed, setSaveFailed] = useState(false)
  const cardRef = useRef<HTMLDivElement>(null)
  const trapFocus = useFocusTrap(cardRef)

  useEffect(() => {
    if (!open) return
    const previous = document.activeElement
    return () => {
      if (previous instanceof HTMLElement) previous.focus()
    }
  }, [open])

  // Também ao trocar entre o diálogo e o estado de falha: o botão focado some junto com a troca.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `failed` só re-dispara o foco ao trocar de visão
  useEffect(() => {
    if (open) cardRef.current?.focus()
  }, [open, failed])

  if (!open) return null

  const confirm = async () => {
    setSaving(true)
    setSaveFailed(false)
    try {
      const results = await saveLegalAcceptance(version)
      // Sucesso fecha sozinho: `config-changed` atualiza o gate e `needed` vira falso.
      if (results.some((result) => !result.success)) setSaveFailed(true)
    } catch (error) {
      rendererLog.error('legal gate: saving acceptance failed', { error })
      setSaveFailed(true)
    } finally {
      setSaving(false)
    }
  }

  return (
    <div
      className="fixed inset-0 flex items-center justify-center bg-bg-deep/80 p-4 backdrop-blur-sm"
      style={layerStyle('critical')}
    >
      <motion.div
        ref={cardRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="legal-consent-title"
        tabIndex={-1}
        onKeyDown={trapFocus}
        initial={{ scale: 0.95, opacity: 0, y: 20 }}
        animate={{ scale: 1, opacity: 1, y: 0 }}
        transition={{ type: 'spring', stiffness: 300, damping: 25 }}
        className="w-full max-w-lg max-h-[calc(100vh-2rem)] overflow-y-auto rounded-[22px] border border-border-subtle bg-bg-deep p-3.5 shadow-xl"
      >
        {failed ? (
          <section
            aria-labelledby="legal-consent-title"
            className="rounded-2xl border border-border-subtle bg-surface-raise p-3.5"
          >
            <h3 id="legal-consent-title" className="flex items-center gap-2 text-sm font-semibold text-text-main">
              <Scale size={16} aria-hidden="true" className="text-accent" />
              {t('legal.consent.title')}
            </h3>
            <ErrorBanner message={t('settings.about.loadError')} />
            <LegalQuitButton />
          </section>
        ) : (
          <LegalConsent
            accepted={accepted}
            onAcceptedChange={setAccepted}
            ageConfirmed={ageConfirmed}
            onAgeConfirmedChange={setAgeConfirmed}
            disabled={saving}
            updated={updated}
          />
        )}
        {saveFailed && <ErrorBanner message={t('legal.consent.saveError')} />}
        <div className="mt-3 flex justify-end">
          {failed ? (
            <PrimaryButton onClick={retry}>{t('button.retry')}</PrimaryButton>
          ) : (
            <PrimaryButton onClick={() => void confirm()} disabled={!accepted || !ageConfirmed || saving}>
              {t('legal.consent.continue')}
            </PrimaryButton>
          )}
        </div>
      </motion.div>
    </div>
  )
}
