import { memo } from 'react'

export { ModelBadges, VendorBadge } from './openrouter-model-badges'
export { ModelCard } from './openrouter-model-card'
export { ModelPrice, PriceCell } from './openrouter-model-price'

export const FilterChip = memo(function FilterChip({
  active,
  icon,
  label,
  onClick,
}: {
  active: boolean
  icon: React.ReactNode
  label: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`inline-flex items-center gap-1.5 rounded-xl border px-3 py-1.5 text-xs font-bold transition ${
        active
          ? 'border-warn-rim bg-warn-soft text-warn shadow-xs'
          : 'border-border-subtle bg-surface-raise/40 text-text-muted hover:border-border-strong hover:bg-surface-raise hover:text-text-main'
      }`}
    >
      {icon}
      {label}
    </button>
  )
})
