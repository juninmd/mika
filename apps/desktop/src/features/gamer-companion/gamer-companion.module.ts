import { defineModule } from '@mika/sdk/contracts'

export const GamerCompanionModule = defineModule({
  id: 'gamer-companion',
  title: 'gamer-companion',
  layer: 'product',
  source: 'apps/desktop/src/features/gamer-companion',
  rust: 'modules::gamer',
  contract: [],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
})
