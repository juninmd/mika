import { defineModule } from '@mika/sdk/contracts'

export const MobileShellModule = defineModule({
  id: 'mobile-shell',
  title: 'Protótipo Android: avatar VRM e chat remoto',
  layer: 'platform',
  source: 'apps/mobile/src',
  contract: ['apps/mobile/src/chat.service.ts'],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
  frozen: { at: '2026-11-23', hash: '79a9f87760dfc4f0' },
})
