import type { LLMModelCatalogItem } from '@mika/sdk/contracts'
import { getLocale } from '@mika/sdk/i18n'
import { dedupeBy } from '@mika/sdk/utils/dedupe'

export type SortBy = 'name' | 'context' | 'price'

export type ModelFilter = {
  query: string
  vendor: string
  onlyFree: boolean
  onlyTools: boolean
  onlyVision: boolean
  sortBy: SortBy
}

export function filterAndSortModels(
  models: LLMModelCatalogItem[],
  { query, vendor, onlyFree, onlyTools, onlyVision, sortBy }: ModelFilter,
): LLMModelCatalogItem[] {
  const q = query.trim().toLowerCase()
  const filtered = models.filter((m) => {
    const s =
      !q ||
      m.name.toLowerCase().includes(q) ||
      m.id.toLowerCase().includes(q) ||
      (m.vendor ?? '').toLowerCase().includes(q)
    const v = vendor === 'all' || m.vendor === vendor
    const f = !onlyFree || Boolean(m.isFree)
    const t = !onlyTools || Boolean(m.supportsTools)
    const vi = !onlyVision || Boolean(m.supportsVision)
    return s && v && f && t && vi
  })
  switch (sortBy) {
    case 'context':
      return filtered.sort((a, b) => (b.contextWindow ?? 0) - (a.contextWindow ?? 0))
    case 'price':
      return filtered.sort((a, b) => {
        if (a.isFree && !b.isFree) return -1
        if (!a.isFree && b.isFree) return 1
        return (
          Number(a.promptPrice ?? 0) +
          Number(a.completionPrice ?? 0) -
          (Number(b.promptPrice ?? 0) + Number(b.completionPrice ?? 0))
        )
      })
    default:
      return filtered.sort((a, b) => a.name.localeCompare(b.name, getLocale()))
  }
}

export function vendorOptions(models: LLMModelCatalogItem[]): string[] {
  const vendorNames = models.map((m) => m.vendor).filter((v): v is string => Boolean(v))
  return ['all', ...dedupeBy(vendorNames, (v) => v)]
}
