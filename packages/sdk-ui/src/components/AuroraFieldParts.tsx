import { t } from '@mika/sdk/i18n'
import { Check, Eye, EyeOff, Search } from 'lucide-react'

export function FieldNotes({ description, saveError, warning, warningId }: FieldNotesProps) {
  return (
    <>
      {saveError && (
        <p role="alert" className="text-caption leading-snug text-danger">
          {saveError || t('sdkUi.saveError')}
        </p>
      )}

      {warning && (
        <p
          id={warningId}
          role="alert"
          className="rounded border border-warn-rim bg-warn-soft px-2 py-1 text-caption leading-snug text-warn"
        >
          {warning}
        </p>
      )}

      {description && <p className="text-caption leading-snug text-text-muted">{description}</p>}
    </>
  )
}

export function FieldHeader({ id, label, badge, saved, dirty, hideLabel }: FieldHeaderProps) {
  if (hideLabel) {
    return (
      <label htmlFor={id} className="sr-only">
        {label}
      </label>
    )
  }
  return (
    <div className="flex items-center gap-1.5">
      <label htmlFor={id} className="text-[13px] font-medium text-text-main">
        {label}
      </label>
      {badge && (
        <span className="rounded border border-info-rim bg-info-soft px-1.5 py-px text-micro font-medium uppercase tracking-[0.1em] text-info">
          {badge}
        </span>
      )}
      {saved && (
        <span className="ml-auto flex items-center gap-1 text-caption font-medium text-ok">
          <Check size={10} aria-hidden="true" /> {t('sdkUi.auroraField.saved')}
        </span>
      )}
      {dirty && !saved && (
        <span
          className="ml-auto h-1.5 w-1.5 animate-pulse rounded-full bg-accent"
          role="status"
          aria-label={t('sdkUi.auroraField.modifiedUnsaved')}
        />
      )}
    </div>
  )
}

interface FieldHeaderProps {
  id: string
  label: string
  badge?: string
  saved: boolean
  dirty: boolean
  hideLabel?: boolean
}

/** Ícone de busca ou botão de revelar senha, sobreposto ao campo. */
export function FieldAdornment({
  type,
  showPw,
  onTogglePw,
}: {
  type: string
  showPw: boolean
  onTogglePw: () => void
}) {
  if (type === 'password') return <PasswordReveal shown={showPw} onToggle={onTogglePw} />
  if (type !== 'search') return null
  return (
    <Search
      size={13}
      aria-hidden="true"
      className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-text-muted"
    />
  )
}

function PasswordReveal({ shown, onToggle }: { shown: boolean; onToggle: () => void }) {
  return (
    <button
      type="button"
      aria-label={shown ? t('sdkUi.auroraField.hidePassword') : t('sdkUi.auroraField.showPassword')}
      onClick={onToggle}
      className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1 text-text-muted transition-colors hover:bg-surface-raise hover:text-text-main focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus"
    >
      {shown ? <EyeOff size={14} aria-hidden="true" /> : <Eye size={14} aria-hidden="true" />}
    </button>
  )
}

interface FieldNotesProps {
  description?: string
  saveError: string | null
  warning?: string
  warningId: string
}
