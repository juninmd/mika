import { AuroraField } from '@mika/sdk-ui/settings'
import { X as XIcon } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { ConfirmButton } from './RichSettingBlocks'
import type { getBulkTogglePlan } from './settings-utils'

type ModulesToolbarProps = {
  bulkSaving: boolean
  bulkTogglePlan: ReturnType<typeof getBulkTogglePlan>
  search: string
  setSearch: (value: string) => void
  toggleAll: (nextValue: boolean) => Promise<void>
}

export function ModulesToolbar({ bulkSaving, bulkTogglePlan, search, setSearch, toggleAll }: ModulesToolbarProps) {
  const { t } = useI18n()
  return (
    <div className="flex items-center gap-3">
      <div className="flex items-center gap-3 mr-2">
        <button
          type="button"
          onClick={() => toggleAll(true)}
          disabled={bulkSaving || bulkTogglePlan.paths.length === 0}
          aria-busy={bulkSaving}
          title={t('settings.modules.toolbar.enableAllTitle').replace('{{count}}', String(bulkTogglePlan.paths.length))}
          className="inline-flex min-h-8 items-center rounded border border-border-subtle bg-surface-raise px-2.5 py-1.5 text-caption font-semibold text-text-main transition-colors hover:border-border-strong hover:bg-surface-raise-strong disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          {t('settings.modules.toolbar.enableAll').replace('{{count}}', String(bulkTogglePlan.paths.length))}
        </button>
        <span aria-hidden="true" className="h-4 w-px bg-surface-raise-strong" />
        <ConfirmButton
          ariaLabel={t('settings.modules.toolbar.disableAllAriaLabel').replace(
            '{{count}}',
            String(bulkTogglePlan.paths.length),
          )}
          label={t('settings.modules.toolbar.disableAll').replace('{{count}}', String(bulkTogglePlan.paths.length))}
          confirmLabel={t('settings.modules.toolbar.disableConfirmLabel')}
          disabled={bulkSaving || bulkTogglePlan.paths.length === 0}
          onConfirm={() => void toggleAll(false)}
          className="font-semibold"
        />
      </div>
      <div className="w-48">
        <AuroraField
          label={t('settings.modules.toolbar.searchLabel')}
          hideLabel
          type="search"
          value={search}
          onChange={setSearch}
          placeholder={t('settings.modules.toolbar.searchPlaceholder')}
          trailing={
            search ? (
              <button
                type="button"
                onClick={() => setSearch('')}
                aria-label={t('settings.modelPicker.clearSearchAriaLabel')}
                className="flex h-9 w-9 items-center justify-center rounded-lg border border-border-subtle bg-surface-raise text-text-subtle hover:text-text-main focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus"
              >
                <XIcon className="size-3.5" aria-hidden="true" />
              </button>
            ) : undefined
          }
        />
      </div>
    </div>
  )
}
