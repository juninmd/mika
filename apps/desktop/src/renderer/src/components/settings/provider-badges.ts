import type { TranslationDict } from '@mika/sdk/i18n'
import type { PillVariant, ProviderBadge } from '@mika/sdk-ui/settings'
import type { Translate } from '../../hooks/useI18n'

export type BadgeKind = 'local' | 'cloud' | 'apiKey' | 'gpu' | 'streaming' | 'neural' | 'recommended'

const BADGES: Record<BadgeKind, { key: keyof TranslationDict; variant: PillVariant }> = {
  local: { key: 'settings.provider.badge.local', variant: 'ok' },
  cloud: { key: 'settings.provider.badge.cloud', variant: 'info' },
  apiKey: { key: 'settings.provider.badge.apiKey', variant: 'warn' },
  gpu: { key: 'settings.provider.badge.gpu', variant: 'accent' },
  streaming: { key: 'settings.provider.badge.streaming', variant: 'accent' },
  neural: { key: 'settings.provider.badge.neural', variant: 'neutral' },
  recommended: { key: 'settings.provider.badge.recommended', variant: 'new' },
}

/** Same vocabulary and colours on every provider card, whatever the tab. */
export function providerBadges(t: Translate, ...kinds: BadgeKind[]): ProviderBadge[] {
  return kinds.map((kind) => ({ label: t(BADGES[kind].key), variant: BADGES[kind].variant }))
}
