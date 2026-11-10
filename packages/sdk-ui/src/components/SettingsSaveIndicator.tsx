import { t } from '@mika/sdk/i18n'
import { AnimatePresence, motion } from 'framer-motion'
import { Check, Loader2, X } from 'lucide-react'
import { useEffect, useState } from 'react'

interface SettingsSaveIndicatorProps {
  isSaving: boolean
  isSaved: boolean
  hasError: boolean
  error?: string | null
}

function SavingIndicator() {
  return (
    <motion.div
      key="saving"
      role="status"
      aria-live="polite"
      initial={{ opacity: 0, x: 4 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: -4 }}
      className="inline-flex items-center gap-1.5 rounded-full border border-accent-rim bg-accent-soft px-2 py-0.5 text-caption font-medium text-accent"
    >
      <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden="true" />
      <span>{t('sdkUi.saveIndicator.saving')}</span>
    </motion.div>
  )
}

function SavedIndicator() {
  return (
    <motion.div
      key="saved"
      role="status"
      aria-live="polite"
      initial={{ opacity: 0, scale: 0.8 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.8 }}
      className="inline-flex items-center gap-1.5 rounded-full border border-ok-rim bg-ok-soft px-2 py-0.5 text-caption font-medium text-ok"
    >
      <Check className="h-3.5 w-3.5" aria-hidden="true" />
      <span>{t('sdkUi.auroraField.saved')}</span>
    </motion.div>
  )
}

function ErrorIndicator({ error }: { error?: string | null }) {
  return (
    <motion.div
      key="error"
      role="alert"
      aria-live="assertive"
      aria-label={error ? t('sdkUi.saveIndicator.errorWith', { error }) : t('sdkUi.statusPill.error')}
      initial={{ opacity: 0, x: 4 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: -4 }}
      className="inline-flex items-center gap-1.5 rounded-full border border-danger-rim bg-danger-soft px-2 py-0.5 text-caption font-medium text-danger"
      title={error ?? undefined}
    >
      <X className="h-3.5 w-3.5" aria-hidden="true" />
      <span>{t('sdkUi.statusPill.error')}</span>
    </motion.div>
  )
}

export function SettingsSaveIndicator({ isSaving, isSaved, hasError, error }: SettingsSaveIndicatorProps) {
  const [visible, setVisible] = useState(true)

  useEffect(() => {
    if (isSaved && !isSaving) {
      const t = setTimeout(() => setVisible(false), 3000)
      return () => clearTimeout(t)
    }
    setVisible(true)
  }, [isSaved, isSaving])

  return (
    <AnimatePresence mode="wait">
      {!visible ? null : isSaving ? (
        <SavingIndicator />
      ) : isSaved ? (
        <SavedIndicator />
      ) : hasError ? (
        <ErrorIndicator error={error} />
      ) : null}
    </AnimatePresence>
  )
}
