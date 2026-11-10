import type { Dispatch, SetStateAction } from 'react'
import { CheckIcon, CoinsIcon, EyeIcon, WrenchIcon } from '../../src/components/Icons'
import { useI18n } from '../../src/hooks/useI18n'
import { FilterChip } from './openrouter-panel-components'

type Toggle = Dispatch<SetStateAction<boolean>>

type CapabilityFiltersProps = {
  onlyFree: boolean
  onlyTools: boolean
  onlyVision: boolean
  setOnlyFree: Toggle
  setOnlyTools: Toggle
  setOnlyVision: Toggle
}

export function CapabilityFilters({
  onlyFree,
  onlyTools,
  onlyVision,
  setOnlyFree,
  setOnlyTools,
  setOnlyVision,
}: CapabilityFiltersProps) {
  const { t } = useI18n()
  return (
    <div className="mt-2 flex flex-wrap gap-2">
      <FilterChip
        icon={<CoinsIcon className="h-3 w-3" />}
        label={t('settings.openrouter.panel.filterFree')}
        active={onlyFree}
        onClick={() => setOnlyFree((v) => !v)}
      />
      <FilterChip
        icon={<WrenchIcon className="h-3 w-3" />}
        label={t('settings.openrouter.panel.filterTools')}
        active={onlyTools}
        onClick={() => setOnlyTools((v) => !v)}
      />
      <FilterChip
        icon={<EyeIcon className="h-3 w-3" />}
        label={t('settings.openrouter.panel.filterVision')}
        active={onlyVision}
        onClick={() => setOnlyVision((v) => !v)}
      />
    </div>
  )
}

export function ActiveModelBadge({ activeModel }: { activeModel: string }) {
  const { t } = useI18n()
  return (
    <div className="flex items-center gap-3 rounded-2xl border border-warn-rim bg-warn/[0.06] p-4">
      <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-warn-soft text-warn">
        <CheckIcon className="h-5 w-5" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="vt-label text-warn">{t('settings.openrouter.panel.activeModel')}</div>
        <div className="truncate font-mono text-sm font-medium text-text-main">
          {activeModel || t('settings.openrouter.panel.noneSelected')}
        </div>
      </div>
    </div>
  )
}
