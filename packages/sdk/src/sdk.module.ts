import { defineModule } from '@mika/sdk/contracts'

export const SdkModule = defineModule({
  id: 'sdk',
  title: 'SDK',
  layer: 'platform',
  profile: 'contract',
  source: 'packages/sdk/src',
  contract: ['packages/sdk/src/contracts.ts'],
  deps: ['EventBus'],
  consumes: [],
  emits: [],
  config: [],
  frozen: { at: '2026-09-24', hash: '65d0586d5991ee80' },
})
