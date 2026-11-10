import { GlassToggle } from '@mika/sdk-ui/settings'
import { CheckCircle2, Loader2, PlayCircle, XCircle } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import type { useFeatureValidation } from './useFeatureValidation'

export function ModuleToggleButton({
  enabled,
  label,
  saving,
  onToggle,
}: {
  enabled: boolean
  label: string
  saving: boolean
  onToggle: () => void
}) {
  const { t } = useI18n()
  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: só barra o clique de chegar ao card (role="button") por trás; o controle real é o GlassToggle dentro.
    <div
      title={t(enabled ? 'settings.moduleCard.disable' : 'settings.moduleCard.enable', { label })}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') e.stopPropagation()
      }}
    >
      <GlassToggle label={label} hideLabel value={enabled} disabled={saving} onChange={onToggle} />
    </div>
  )
}

export function ModuleValidationButton({
  label,
  validation,
}: {
  label: string
  validation: ReturnType<typeof useFeatureValidation>
}) {
  const { t } = useI18n()
  const { running, state, validate } = validation
  const Icon =
    state.status === 'success' ? CheckCircle2 : state.status === 'error' ? XCircle : running ? Loader2 : PlayCircle
  const statusClass =
    state.status === 'success'
      ? 'border-accent-rim/40 bg-accent-soft text-accent'
      : state.status === 'error'
        ? 'border-danger-rim bg-danger-soft text-danger'
        : 'border-border-subtle bg-surface-raise text-text-muted hover:border-border-strong hover:bg-surface-raise-strong hover:text-text-main'

  return (
    <button
      type="button"
      aria-label={t('settings.moduleCard.validate', { label })}
      disabled={running}
      title={state.message}
      onClick={(event) => {
        event.stopPropagation()
        void validate()
      }}
      className={`flex h-8 w-8 items-center justify-center rounded-lg border transition hover:scale-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-wait disabled:opacity-50 ${statusClass}`}
    >
      <Icon className={`size-3.5 ${running ? 'animate-spin' : ''}`} aria-hidden="true" />
    </button>
  )
}
