import type { usePluginContext } from '@mika/sdk-ui'
import type { Translate } from '../../../../renderer/src/hooks/useI18n'
import { useUIStore } from '../../../../renderer/src/store/useUIStore'
import type { CardPerson } from '../components/person-card'

export function reportPersonCatalogError(error: string | undefined, fallback: string): void {
  useUIStore.getState().addNotification('error', error ?? fallback)
}

export const FILTERS = ['all', 'politician', 'real_person', 'fictional_character', 'streamer', 'vtuber', 'athlete']

export const CATALOG_REFRESH_MS = 3000

export function signatureOf(persons: CardPerson[]): string {
  return persons.map((p) => `${p.id}:${p.lastSeenAt ?? 0}`).join('|')
}

export function mergeModeLabel(t: Translate, merging: boolean, mergeTarget: CardPerson | null): string {
  if (!merging) return t('settings.personCatalog.crud.mergeDefaultLabel')
  return mergeTarget
    ? `${t('settings.personCatalog.crud.mergeTargetPrefix')} ${mergeTarget.primaryName} ${t('settings.personCatalog.crud.mergeTargetSuffix')}`
    : t('settings.personCatalog.crud.mergePickLabel')
}

export async function mergePersons(
  api: ReturnType<typeof usePluginContext>['api'],
  target: CardPerson,
  source: CardPerson,
): Promise<{ success: boolean; error?: string } | undefined> {
  return api.rpc.call<{ success: boolean; error?: string }>('person_catalog_merge', {
    targetId: target.id,
    sourceId: source.id,
    confirm: true,
  })
}
