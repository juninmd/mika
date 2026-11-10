import { defineModule } from '@mika/sdk/contracts'

export const ComputerUseModule = defineModule({
  id: 'computer-use',
  title: 'computer-use',
  layer: 'product',
  source: 'apps/desktop/src/features/computer-use',
  rust: 'modules::features::computer_use',
  contract: [],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
})
