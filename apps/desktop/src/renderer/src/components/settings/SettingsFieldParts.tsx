import type { SettingsField } from '@mika/sdk/contracts'
import { AnimatePresence, motion } from 'framer-motion'
import { useI18n } from '../../hooks/useI18n'
import { openExternalLink } from '../../lib/shell-links'
import { SaveIcon } from '../Icons'

function saveButtonClass(saved: boolean, isDirty: boolean): string {
  const baseClass =
    'inline-flex min-h-11 w-11 shrink-0 items-center justify-center rounded-lg border transition disabled:opacity-50 active:scale-95 focus-visible:ring-2 focus-visible:ring-focus focus-visible:outline-none'
  if (saved) return `${baseClass} border-ok-rim/50 bg-ok-soft text-ok`
  if (isDirty) return `${baseClass} border-accent-rim bg-accent-soft text-accent hover:bg-accent/25`
  return `${baseClass} border-border-subtle bg-surface-raise text-text-muted hover:border-border-strong hover:bg-surface-raise-strong hover:text-text-main`
}

function SaveButtonIcon({ saving, saved }: { saving: boolean; saved: boolean }) {
  if (saving) {
    return (
      <motion.span
        key="spin"
        initial={{ opacity: 0, scale: 0.8 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.8 }}
        className="h-4 w-4 animate-spin rounded-full border-2 border-border-subtle border-t-accent"
      />
    )
  }
  if (saved) {
    return (
      <motion.svg
        key="check"
        initial={{ opacity: 0, scale: 0.5 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.8 }}
        transition={{ type: 'spring', stiffness: 500, damping: 25 }}
        viewBox="0 0 20 20"
        fill="none"
        stroke="currentColor"
        strokeWidth={2.5}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
        className="h-5 w-5"
      >
        <path d="M4 10l4.5 4.5L16 6" />
      </motion.svg>
    )
  }
  return (
    <motion.span
      key="save"
      initial={{ opacity: 0, scale: 0.8 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.8 }}
    >
      <SaveIcon className="h-5 w-5" aria-hidden="true" />
    </motion.span>
  )
}

export function FieldSaveButton({
  label,
  saving,
  saved,
  isDirty,
  onCommit,
}: {
  label: string
  saving: boolean
  saved: boolean
  isDirty: boolean
  onCommit: () => void
}) {
  const { t } = useI18n()
  return (
    <button
      type="button"
      aria-label={`${t('button.save')} ${label}`}
      title={t('button.save')}
      disabled={saving || (!isDirty && !saved)}
      className={saveButtonClass(saved, isDirty)}
      onClick={onCommit}
    >
      <AnimatePresence mode="wait" initial={false}>
        <SaveButtonIcon saving={saving} saved={saved} />
      </AnimatePresence>
    </button>
  )
}

export function FieldExamples({
  examples,
  configPath,
  saving,
  onPick,
}: {
  examples: string[]
  configPath: string
  saving: boolean
  onPick: (example: string) => void
}) {
  const { t } = useI18n()
  if (examples.length === 0) return null
  return (
    <div className="flex flex-wrap gap-1.5 pt-1">
      {examples.map((ex) => (
        <button
          key={`${configPath}-example-${ex}`}
          type="button"
          title={t('settings.fields.useExample').replace('{{example}}', ex)}
          disabled={saving}
          className="rounded-md border border-border-subtle bg-surface-raise px-2 py-1 text-caption text-text-muted transition-colors hover:border-accent-rim hover:bg-accent-soft hover:text-accent active:scale-95 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus disabled:opacity-50 disabled:pointer-events-none"
          onClick={() => onPick(ex)}
        >
          {ex}
        </button>
      ))}
    </div>
  )
}

export function FieldRangeAndLink({ field }: { field: SettingsField }) {
  const { t } = useI18n()
  if (field.min === undefined && field.max === undefined && !field.pattern && !field.learnMoreUrl) return null
  return (
    <div className="flex flex-wrap items-center gap-3 text-caption text-text-muted pt-1">
      {field.min !== undefined && field.max !== undefined && (
        <span>
          {t('settings.fields.range')}: {field.min} — {field.max}
        </span>
      )}
      {field.learnMoreUrl && (
        <button
          type="button"
          onClick={() => void openExternalLink(field.learnMoreUrl ?? '', t('settings.providerCredentials.getKeyError'))}
          className="rounded text-accent hover:underline underline-offset-4 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus"
        >
          {t('settings.fields.documentation')}
        </button>
      )}
    </div>
  )
}
