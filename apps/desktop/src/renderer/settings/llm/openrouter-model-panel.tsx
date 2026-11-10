import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { toDisplayError } from '@mika/sdk-ui'
import { AuroraField, GlassSelect } from '@mika/sdk-ui/settings'
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from 'react'
import { SparkIcon } from '../../src/components/Icons'
import { OpenRouterQuotaLine } from '../../src/components/settings/OpenRouterQuotaLine'
import { useI18n } from '../../src/hooks/useI18n'
import { capitalizeLocal } from '../../src/lib/format-number'
import { getElectronApi } from '../../src/lib/shell-api'
import { useUIStore } from '../../src/store/useUIStore'
import { ActiveModelBadge, CapabilityFilters } from './openrouter-model-controls'
import { filterAndSortModels, type SortBy, vendorOptions } from './openrouter-model-filter'
import { ModelList } from './openrouter-model-list'
import { useOpenRouterQuota } from './use-openrouter-quota'
import { useResponsiveCols } from './use-responsive-cols'

type OpenRouterModelPanelProps = {
  activeModel: string
  apiKey: string
  onSave: (path: string, value: unknown) => Promise<void>
}

export function OpenRouterModelPanel({ activeModel, apiKey, onSave }: OpenRouterModelPanelProps) {
  const { t } = useI18n()
  const [models, setModels] = useState<LLMModelCatalogItem[]>([])
  const [search, setSearch] = useState('')
  const [vendor, setVendor] = useState('all')
  const [onlyFree, setOnlyFree] = useState(false)
  const [onlyTools, setOnlyTools] = useState(false)
  const [onlyVision, setOnlyVision] = useState(false)
  const [sortBy, setSortBy] = useState<SortBy>('name')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const requestRef = useRef<string | null>(null)
  const cols = useResponsiveCols()
  const deferredSearch = useDeferredValue(search)
  const { quota, quotaError } = useOpenRouterQuota(apiKey, t)

  const vendors = useMemo(() => vendorOptions(models), [models])

  const filteredModels = useMemo(() => {
    return filterAndSortModels(models, { query: deferredSearch, vendor, onlyFree, onlyTools, onlyVision, sortBy })
  }, [models, deferredSearch, vendor, onlyFree, onlyTools, onlyVision, sortBy])

  const loadModels = useCallback(async () => {
    const requestKey = apiKey.trim()
    if (!requestKey) return
    requestRef.current = requestKey
    const applyIfCurrent = (apply: () => void) => {
      if (requestRef.current === requestKey) apply()
    }
    setLoading(true)
    setError(null)
    try {
      const r = await getElectronApi().runtime.getLlmModels({ provider: 'openrouter', apiKey: requestKey })
      applyIfCurrent(() => {
        setModels(r.models)
        if (r.error) setError(r.error)
      })
    } catch (e) {
      applyIfCurrent(() => setError(toDisplayError(e, t('settings.openrouter.panel.catalogLoadError'))))
    } finally {
      applyIfCurrent(() => setLoading(false))
    }
  }, [apiKey, t])

  useEffect(() => {
    const timeoutId = setTimeout(() => loadModels(), 400)
    return () => clearTimeout(timeoutId)
  }, [loadModels])

  const selectModel = useCallback(
    async (id: string) => {
      try {
        await onSave('llm.openrouterModel', id)
      } catch (e) {
        useUIStore.getState().addNotification('error', toDisplayError(e, t('sdkUi.saveError')))
      }
    },
    [onSave, t],
  )

  return (
    <section className="rounded-[1.5rem] border border-border-subtle bg-bg-surface p-5 md:p-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-accent-rim bg-accent-soft text-accent">
            <SparkIcon className="h-5 w-5" />
          </div>
          <div>
            <h3 className="text-lg font-black uppercase tracking-tight text-text-main">OpenRouter</h3>
            <p className="vt-label">{t('settings.openrouter.panel.subtitle')}</p>
          </div>
        </div>
        <button
          type="button"
          onClick={() => void loadModels()}
          disabled={loading || !apiKey.trim()}
          className="border border-border-subtle bg-surface-raise text-text-muted transition duration-300 hover:border-primary/40 hover:bg-primary/10 hover:text-text-main active:scale-95 rounded-xl px-4 py-2 text-xs font-bold uppercase tracking-widest disabled:opacity-50"
        >
          {loading ? t('settings.openrouter.panel.syncing') : t('settings.openrouter.panel.refreshCatalog')}
        </button>
      </div>

      <div className="mt-6 space-y-5">
        <div className="grid gap-3 lg:grid-cols-2">
          <div className="rounded-2xl border border-border-subtle bg-black/20 p-4">
            <div className="vt-label mb-2 text-warn">{t('settings.openrouter.panel.apiKeyLabel')}</div>
            {apiKey.trim() ? (
              <>
                <p className="text-sm text-text-main">{t('settings.openrouter.panel.apiKeyConfigured')}</p>
                {quota ? (
                  <OpenRouterQuotaLine quota={quota} />
                ) : quotaError ? (
                  <p className="mt-1 text-xs text-danger">{quotaError}</p>
                ) : (
                  <p className="mt-1 text-xs text-text-muted">{t('settings.openrouter.panel.quotaLoading')}</p>
                )}
              </>
            ) : (
              <p className="text-sm text-text-muted">{t('settings.openrouter.panel.apiKeyMissing')}</p>
            )}
          </div>

          <div className="rounded-2xl border border-border-subtle bg-black/20 p-4">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
              <span className="vt-label text-warn">{t('settings.openrouter.panel.modelCatalog')}</span>
              <span className="inline-flex items-center gap-1 rounded-md border border-border-subtle bg-surface-raise px-2 py-1 text-micro font-extrabold uppercase tracking-wider text-text-muted">
                {filteredModels.length} {t('settings.openrouter.panel.modelsCountSuffix')}
              </span>
            </div>

            <div className="grid gap-2 lg:grid-cols-[1fr_auto_auto]">
              <AuroraField
                label={t('settings.openrouter.panel.searchPlaceholder')}
                hideLabel
                type="search"
                value={search}
                onChange={setSearch}
                placeholder={t('settings.openrouter.panel.searchPlaceholder')}
              />
              <GlassSelect
                label={t('settings.openrouter.panel.vendorFilterLabel')}
                hideLabel
                value={vendor}
                onChange={setVendor}
                options={vendors.map((v) => ({
                  value: v,
                  label: v === 'all' ? t('settings.openrouter.panel.allVendors') : capitalizeLocal(v),
                }))}
              />
              <GlassSelect
                label={t('settings.openrouter.panel.sortByLabel')}
                hideLabel
                value={sortBy}
                onChange={(v) => setSortBy(v as SortBy)}
                options={[
                  { value: 'name', label: t('settings.openrouter.panel.sortName') },
                  { value: 'context', label: t('settings.openrouter.panel.sortContext') },
                  { value: 'price', label: t('settings.openrouter.panel.sortPrice') },
                ]}
              />
            </div>

            <CapabilityFilters
              onlyFree={onlyFree}
              onlyTools={onlyTools}
              onlyVision={onlyVision}
              setOnlyFree={setOnlyFree}
              setOnlyTools={setOnlyTools}
              setOnlyVision={setOnlyVision}
            />
          </div>
        </div>

        <ModelList
          models={filteredModels}
          cols={cols}
          loading={loading}
          search={search}
          activeModel={activeModel}
          onSelect={(id) => void selectModel(id)}
        />

        <ActiveModelBadge activeModel={activeModel} />

        {error && (
          <div
            role="alert"
            className="rounded-lg border border-danger-rim bg-danger-soft px-4 py-3 text-sm font-bold text-danger"
          >
            {error}
          </div>
        )}
      </div>
    </section>
  )
}
