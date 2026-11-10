import { defineModule } from '@mika/sdk/contracts'

export const SchedulerModule = defineModule({
  id: 'scheduler',
  title: 'scheduler',
  layer: 'product',
  source: 'apps/desktop/src/features/scheduler',
  rust: 'modules::features::scheduler',
  contract: [],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
})
