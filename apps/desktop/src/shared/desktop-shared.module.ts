import { defineModule } from '@mika/sdk/contracts'

export const DesktopSharedModule = defineModule({
  id: 'desktop-shared',
  title: 'Contratos compartilhados do desktop',
  layer: 'platform',
  source: 'apps/desktop/src/shared',
  contract: [
    'apps/desktop/src/shared/electron-api-types.ts',
    'apps/desktop/src/shared/mika-event-channels.ts',
    'apps/desktop/src/shared/mika-invoke-channels.ts',
    'apps/desktop/src/shared/plugin-rpc-contract.ts',
  ],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
  frozen: { at: '2026-11-15', hash: '234569def56b91a1' },
})
