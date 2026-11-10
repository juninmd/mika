import type { SettingsField } from '@mika/sdk/contracts'
import { useCopyToClipboard } from '@mika/sdk-ui'
import { AuroraField, GlassSelect, GlassSlider } from '@mika/sdk-ui/settings'
import { useI18n } from '../../hooks/useI18n'
import { CheckIcon, CopyIcon } from '../Icons'
import { resolvePlaceholder } from './settings-field-placeholder'

const BASE_COPY_BUTTON_CLASS =
  'flex h-9 w-9 items-center justify-center rounded-lg border border-border-subtle bg-surface-raise text-text-muted transition hover:text-info focus-visible:ring-2 focus-visible:ring-focus focus-visible:outline-none'

type SettingsFieldInputProps = {
  field: SettingsField
  draft: string
  describedBy?: string
  onDraftChange: (next: string) => void
  /** `next` evita salvar o draft velho quando a escolha e o commit acontecem no mesmo tick. */
  onSubmit: (next?: string) => void
  onCommit?: () => void
  disabled?: boolean
}

function CopyButton({ copied, onCopy }: { copied: boolean; onCopy: () => void }) {
  const { t } = useI18n()
  return (
    <button
      type="button"
      aria-label={copied ? t('button.copied') : t('settings.field.copyToClipboard')}
      title={copied ? t('button.copied') : t('button.copy')}
      onClick={onCopy}
      className={`${BASE_COPY_BUTTON_CLASS} ${copied ? 'bg-ok-soft' : ''}`}
    >
      {copied ? (
        <CheckIcon className="size-3.5 text-ok" aria-hidden="true" />
      ) : (
        <CopyIcon className="size-3.5" aria-hidden="true" />
      )}
    </button>
  )
}

function isRangeField(field: SettingsField): boolean {
  return field.type === 'range' || (field.type === 'number' && field.min !== undefined && field.max !== undefined)
}

// Modos além de 'text' que a AuroraField entende — o resto do SettingsField.type cai em 'text'.
function resolveAuroraType(field: SettingsField): 'text' | 'number' | 'password' | 'url' | 'email' | 'search' {
  if (field.type === 'password') return 'password'
  if (field.type === 'url') return 'url'
  if (field.type === 'number') return 'number'
  if (field.inputMode === 'email') return 'email'
  if (field.inputMode === 'search') return 'search'
  return 'text'
}

export function SettingsFieldInput({
  field,
  draft,
  describedBy,
  onDraftChange,
  onSubmit,
  onCommit,
  disabled = false,
}: SettingsFieldInputProps) {
  const { t } = useI18n()
  const { copied, copy: copyToClipboard } = useCopyToClipboard()
  const canCopy = field.type !== 'boolean' && draft.length > 0
  const copyTrailing = canCopy ? (
    <CopyButton copied={copied} onCopy={() => copyToClipboard(draft).catch(() => {})} />
  ) : undefined

  if (isRangeField(field)) {
    const min = field.min ?? 0
    const max = field.max ?? 100
    return (
      <GlassSlider
        label={field.label}
        value={Number.isFinite(Number.parseFloat(draft)) ? Number(draft) : min}
        min={min}
        max={max}
        step={field.step ?? 1}
        showValue={(v) => `${v}${field.unit ? ` ${field.unit}` : ''}`}
        disabled={disabled}
        onChange={(v) => onDraftChange(String(v))}
        describedBy={describedBy}
        onCommit={(v) => onSubmit(String(v))}
      />
    )
  }

  if (field.type === 'select') {
    return (
      <GlassSelect
        label={field.label}
        hideLabel
        value={draft}
        disabled={disabled}
        options={(field.options ?? []).map((o) => ({ value: String(o.value), label: o.label }))}
        describedBy={describedBy}
        onChange={(next) => {
          onDraftChange(next)
          onSubmit(next)
        }}
      />
    )
  }

  return (
    <AuroraField
      label={field.label}
      hideLabel
      describedBy={describedBy}
      type={field.type === 'textarea' ? 'textarea' : resolveAuroraType(field)}
      rows={field.rows ?? 3}
      value={draft}
      placeholder={resolvePlaceholder(field, t)}
      disabled={disabled}
      onChange={onDraftChange}
      onSubmit={onSubmit}
      onCommit={onCommit}
      trailing={copyTrailing}
    />
  )
}
