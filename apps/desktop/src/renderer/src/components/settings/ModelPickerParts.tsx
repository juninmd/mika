import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { ConfirmButton } from '@mika/sdk-ui/settings'
import { Check, Eye, Wrench } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { formatContext, type ModelCapability } from './model-catalog'

export function ModelRow({
  model,
  selected,
  onPick,
}: {
  model: LLMModelCatalogItem
  selected: boolean
  onPick: () => void
}) {
  const context = formatContext(model.contextWindow)
  return (
    <button
      type="button"
      onClick={onPick}
      aria-pressed={selected}
      className={`flex w-full items-center gap-2 border-b border-border-subtle px-3 py-2 text-left last:border-b-0 transition-colors hover:bg-surface-raise-strong/60 active:scale-[0.99] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${selected ? 'bg-accent-soft' : ''}`}
    >
      <Check className={`h-3.5 w-3.5 shrink-0 ${selected ? 'text-accent-text' : 'opacity-0'}`} aria-hidden="true" />
      <span className="min-w-0 flex-1 truncate text-[12px] text-text-main">{model.name}</span>
      <span className="flex shrink-0 items-center gap-1.5">
        {model.supportsVision ? <Eye className="h-3 w-3 text-info/70" aria-hidden="true" /> : null}
        {model.supportsTools ? <Wrench className="h-3 w-3 text-accent/70" aria-hidden="true" /> : null}
        {model.isFree ? <span className="rounded bg-ok-soft px-1 text-caption font-bold text-ok">FREE</span> : null}
        {context ? <span className="text-caption text-text-muted">{context}</span> : null}
      </span>
    </button>
  )
}

export function CatalogNotices({
  error,
  usingFallback,
  capabilityUnknown,
  capability,
  provider,
  fallbackNote,
}: {
  error: string | null
  usingFallback: boolean
  capabilityUnknown: boolean
  capability: ModelCapability | undefined
  provider: string | undefined
  fallbackNote: string | undefined
}) {
  const { t } = useI18n()
  return (
    <>
      {error ? (
        <p className="text-caption text-danger">
          {error} — {t('settings.modelPicker.errorCredentialHint')}
        </p>
      ) : null}

      {usingFallback ? (
        <p className="rounded-lg border border-warn-rim bg-warn-soft px-3 py-2 text-caption text-warn/90">
          {t('settings.modelPicker.fallbackLabel')} (<span className="font-bold">source: static</span>) —{' '}
          {t('settings.modelPicker.fallbackRealCatalogPrefix')} <span className="font-bold">{provider}</span>{' '}
          {t('settings.modelPicker.fallbackRealCatalogSuffix')}
          {fallbackNote ? ` ${fallbackNote}` : ''}
        </p>
      ) : null}

      {capabilityUnknown ? (
        <p className="text-caption text-text-muted">
          {capability === 'vision'
            ? t('settings.modelPicker.capabilityUnknownVision')
            : t('settings.modelPicker.capabilityUnknownTools')}
        </p>
      ) : null}
    </>
  )
}

export function EmbeddingChangeConfirm({
  pendingModelId,
  onCancel,
  onConfirm,
}: {
  pendingModelId: string | null
  onCancel: () => void
  onConfirm: (modelId: string) => void
}) {
  const { t } = useI18n()
  if (!pendingModelId) return null
  return (
    <div
      role="alertdialog"
      aria-label={t('settings.modelPicker.embeddingChangeAriaLabel')}
      className="flex items-center justify-between gap-3 rounded-lg border border-warn-rim bg-warn-soft px-3 py-2"
    >
      <p className="min-w-0 flex-1 text-caption text-warn">
        {t('settings.llm.panel.embeddingProviderChangePrefix')} <span className="font-bold">{pendingModelId}</span>{' '}
        {t('settings.modelPicker.embeddingChangeSuffix')}
      </p>
      <div className="flex shrink-0 items-center gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="rounded border border-border-subtle px-2 py-1 text-caption font-semibold text-text-muted transition-colors hover:border-border-strong hover:bg-surface-raise hover:text-text-main focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus"
        >
          {t('button.cancel')}
        </button>
        <ConfirmButton
          ariaLabel={`${t('settings.modelPicker.confirmChangeAriaLabelModel')} ${pendingModelId}`}
          label={t('settings.llm.panel.confirmChangeLabel')}
          confirmLabel={t('settings.llm.panel.confirmChangeButton')}
          onConfirm={() => onConfirm(pendingModelId)}
        />
      </div>
    </div>
  )
}
