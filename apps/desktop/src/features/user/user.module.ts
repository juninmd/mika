import { defineModule } from '@mika/sdk/contracts'

export const UserModule = defineModule({
  id: 'user',
  title: 'user',
  layer: 'product',
  source: 'apps/desktop/src/features/user',
  rust: 'modules::features::user',
  contract: [],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
})
