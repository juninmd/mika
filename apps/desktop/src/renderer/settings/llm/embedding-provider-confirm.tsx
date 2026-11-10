import { useI18n } from '../../src/hooks/useI18n'

export function ProviderChangeConfirm({
  pendingProviderId,
  onCancel,
  onConfirm,
}: {
  pendingProviderId: string | null
  onCancel: () => void
  onConfirm: (providerId: string) => void
}) {
  const { t } = useI18n()
  if (!pendingProviderId) return null
  return (
    <div
      role="alertdialog"
      aria-label={t('settings.llm.panel.embeddingProviderChangeAriaLabel')}
      className="flex items-center justify-between gap-3 rounded-lg border border-warn-rim bg-warn-soft px-3 py-2"
    >
      <p className="min-w-0 flex-1 text-caption text-warn">
        {t('settings.llm.panel.embeddingProviderChangePrefix')} <span className="font-bold">{pendingProviderId}</span>{' '}
        {t('settings.llm.panel.embeddingProviderChangeSuffix')}
      </p>
      <div className="flex shrink-0 items-center gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="rounded border border-border-subtle px-2 py-1 text-caption font-semibold text-text-muted transition-colors hover:border-border-strong hover:bg-surface-raise hover:text-text-main focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus"
        >
          {t('button.cancel')}
        </button>
        <button
          type="button"
          onClick={() => onConfirm(pendingProviderId)}
          aria-label={`${t('settings.llm.panel.confirmChangeAriaLabelProvider')} ${pendingProviderId}`}
          className="rounded bg-danger px-2 py-1 text-caption font-bold text-white transition-[filter] hover:brightness-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
        >
          {t('settings.llm.panel.confirmChangeLabel')}
        </button>
      </div>
    </div>
  )
}
