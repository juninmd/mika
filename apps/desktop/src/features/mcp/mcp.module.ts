import { defineModule } from '@mika/sdk/contracts'

export const McpModule = defineModule({
  id: 'mcp',
  title: 'mcp',
  layer: 'product',
  source: 'apps/desktop/src/features/mcp',
  rust: 'modules::mcp',
  contract: [],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
})
