import { defineModule } from '@mika/sdk/contracts'

export const PersonCatalogModule = defineModule({
  id: 'person-catalog',
  title: 'person-catalog',
  layer: 'product',
  source: 'apps/desktop/src/features/person-catalog',
  rust: 'modules::features::person_catalog',
  contract: [],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
})
