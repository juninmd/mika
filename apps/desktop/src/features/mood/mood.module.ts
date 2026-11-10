import { defineModule } from '@mika/sdk/contracts'

export const MoodModule = defineModule({
  id: 'mood',
  title: 'mood',
  layer: 'product',
  source: 'apps/desktop/src/features/mood',
  rust: 'modules::features::mood',
  contract: [],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
})
