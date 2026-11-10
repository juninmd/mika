import { defineModule } from '@mika/sdk/contracts'

export const PrivacyModule = defineModule({
  id: 'privacy',
  title: 'privacy',
  layer: 'product',
  source: 'apps/desktop/src/features/privacy',
  rust: 'modules::features::privacy',
  contract: [],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
})
