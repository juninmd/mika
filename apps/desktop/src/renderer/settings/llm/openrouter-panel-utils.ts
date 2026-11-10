import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { formatLocalDate } from '@mika/sdk-ui'

export function formatPricePer1M(priceStr?: string): string | null {
  if (!priceStr) return null
  const price = Number(priceStr)
  if (!Number.isFinite(price) || price <= 0) return null
  if (price < 0.000001) return `$${(price * 1_000_000).toFixed(4)}`
  return `$${(price * 1_000_000).toFixed(2)}`
}

export function formatContextWindow(model: LLMModelCatalogItem): string {
  if (!model.contextWindow) return ''
  const k = model.contextWindow / 1024
  return k >= 1 ? `${Math.round(k)}K` : `${model.contextWindow}`
}

export function formatKnowledgeCutoff(dateStr: string | undefined, locale: string): string {
  if (!dateStr) return ''
  const d = new Date(dateStr)
  if (Number.isNaN(d.getTime())) return dateStr.slice(0, 7)
  return formatLocalDate(d, locale, { month: 'short', year: 'numeric' })
}
