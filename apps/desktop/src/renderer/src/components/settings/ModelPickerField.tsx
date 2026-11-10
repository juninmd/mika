import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { toDisplayError } from '@mika/sdk-ui'
import { AuroraField, GlassToggle } from '@mika/sdk-ui/settings'
import { AlertTriangle, RefreshCw, X } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { useUIStore } from '../../store/useUIStore'
import { CatalogNotices, EmbeddingChangeConfirm, ModelRow } from './ModelPickerParts'
import type { ModelCapability, ModelCatalogKind } from './model-catalog'
import { hasCapability, isEmbeddingModelChange, resolvePlaceholder, useModelCatalog } from './model-catalog'

export type { ModelCatalogKind }

interface Props {
  label: string
  description?: string

  provider: string

  value: string
  configPath: string
  onSave: (path: string, value: unknown) => Promise<void>

  capability?: ModelCapability

  kind?: ModelCatalogKind
  placeholder?: string

  fallbackModels?: readonly LLMModelCatalogItem[]

  fallbackNote?: string
}

export function ModelPickerField({
  label,
  description,
  provider,
  value,
  configPath,
  onSave,
  capability,
  kind = 'chat',
  placeholder,
  fallbackModels,
  fallbackNote,
}: Props) {
  const { t } = useI18n()
  const resolvedPlaceholder = resolvePlaceholder(placeholder, t('settings.modelPicker.placeholder'))
  const [search, setSearch] = useState('')
  const [showAll, setShowAll] = useState(false)
  const [pendingModelId, setPendingModelId] = useState<string | null>(null)
  const { models, loading, error, usingFallback, reload } = useModelCatalog(provider, kind, fallbackModels)

  const persistModel = useCallback(
    async (modelId: string) => {
      try {
        await onSave(configPath, modelId)
      } catch (e) {
        useUIStore.getState().addNotification('error', toDisplayError(e, t('sdkUi.saveError')))
      }
    },
    [onSave, configPath, t],
  )

  const handlePick = useCallback(
    (modelId: string) => {
      if (isEmbeddingModelChange(kind, modelId, value)) {
        setPendingModelId(modelId)
        return
      }
      void persistModel(modelId)
    },
    [kind, value, persistModel],
  )

  const capabilityMatches = useMemo(() => models.filter((m) => hasCapability(m, capability)), [models, capability])

  const providerReportsCapability = capabilityMatches.length > 0
  const filterActive = Boolean(capability) && providerReportsCapability && !showAll

  const visible = useMemo(() => {
    const pool = filterActive ? capabilityMatches : models
    const q = search.trim().toLowerCase()
    const filtered = q ? pool.filter((m) => `${m.id} ${m.name}`.toLowerCase().includes(q)) : pool
    return filtered.slice(0, 200)
  }, [models, capabilityMatches, filterActive, search])

  const current = models.find((m) => m.id === value)
  const orphanValue = Boolean(value) && models.length > 0 && !current

  const hiddenByFilter = filterActive ? models.length - capabilityMatches.length : 0
  const capabilityUnknown = Boolean(capability) && models.length > 0 && !providerReportsCapability

  return (
    <div className="space-y-2">
      <div className="flex items-baseline justify-between gap-3">
        <div>
          <span className="text-sm font-semibold text-text-main">{label}</span>
          {description ? <p className="mt-0.5 text-caption text-text-muted">{description}</p> : null}
        </div>
        <button
          type="button"
          onClick={reload}
          disabled={loading}
          className="flex shrink-0 items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-raise px-2 py-1 text-caption font-semibold text-text-muted transition active:scale-95 hover:border-border-strong hover:bg-surface-raise-strong hover:text-text-main disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <RefreshCw className={`h-3 w-3 ${loading ? 'animate-spin' : ''}`} aria-hidden="true" />
          {loading ? t('settings.modelPicker.refreshing') : t('settings.modelPicker.refresh')}
        </button>
      </div>

      {orphanValue ? (
        <div role="alert" className="flex items-start gap-2 rounded-lg border border-warn/40 bg-warn-soft px-3 py-2">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-warn" aria-hidden="true" />
          <p className="text-caption text-warn">
            <span className="font-bold">{value}</span> {t('settings.modelPicker.orphanNotInCatalog')}{' '}
            <span className="font-bold">{provider}</span>. {t('settings.modelPicker.orphanProbablyOtherProvider')}
          </p>
        </div>
      ) : null}

      <AuroraField
        label={resolvedPlaceholder}
        hideLabel
        type="search"
        value={search}
        onChange={setSearch}
        placeholder={resolvedPlaceholder}
        trailing={
          search ? (
            <button
              type="button"
              onClick={() => setSearch('')}
              aria-label={t('settings.modelPicker.clearSearchAriaLabel')}
              className="flex h-9 w-9 items-center justify-center rounded-lg border border-border-subtle bg-surface-raise text-text-subtle hover:text-text-main focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus"
            >
              <X aria-hidden="true" size={16} />
            </button>
          ) : undefined
        }
      />

      <CatalogNotices
        error={error}
        usingFallback={usingFallback}
        capabilityUnknown={capabilityUnknown}
        capability={capability}
        provider={provider}
        fallbackNote={fallbackNote}
      />

      <EmbeddingChangeConfirm
        pendingModelId={pendingModelId}
        onCancel={() => setPendingModelId(null)}
        onConfirm={(modelId) => {
          void persistModel(modelId)
          setPendingModelId(null)
        }}
      />

      {hiddenByFilter > 0 ? (
        <GlassToggle
          label={`${t('settings.modelPicker.showAllLabel')} (${hiddenByFilter} ${t('settings.modelPicker.hiddenNoSupportFor')} ${
            capability === 'vision'
              ? t('settings.modelPicker.capabilityVision')
              : t('settings.modelPicker.capabilityTools')
          } ${t('settings.modelPicker.hiddenHidden')})`}
          value={showAll}
          onChange={setShowAll}
        />
      ) : null}

      <div className="max-h-64 overflow-y-auto rounded-lg border border-border-subtle bg-surface-raise/20">
        {visible.length === 0 ? (
          <p role="status" className="px-3 py-4 text-center text-caption text-text-muted">
            {loading ? t('settings.modelPicker.loadingCatalog') : t('settings.modelPicker.noModelsFound')}
          </p>
        ) : (
          visible.map((model) => (
            <ModelRow key={model.id} model={model} selected={model.id === value} onPick={() => handlePick(model.id)} />
          ))
        )}
      </div>
    </div>
  )
}
