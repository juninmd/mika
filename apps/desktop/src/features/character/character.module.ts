import { defineModule } from '@mika/sdk/contracts'

export const CharacterModule = defineModule({
  id: 'character',
  title: 'character',
  layer: 'product',
  source: 'apps/desktop/src/features/character',
  rust: 'modules::features::character',
  contract: [],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
})
