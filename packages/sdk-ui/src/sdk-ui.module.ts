import { defineModule } from '@mika/sdk/contracts'

export const SdkUiModule = defineModule({
  id: 'sdk-ui',
  title: 'SDK de interface',
  layer: 'product',
  source: 'packages/sdk-ui/src',
  contract: ['packages/sdk-ui/src/index.tsx', 'packages/sdk-ui/src/settings.ts'],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
  frozen: { at: '2026-10-06', hash: '5e481c21177d9eff' },
})
