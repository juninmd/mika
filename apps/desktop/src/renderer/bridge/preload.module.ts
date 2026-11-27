import { defineModule } from '@mika/sdk/contracts'

export const PreloadModule = defineModule({
  id: 'preload',
  title: 'Ponte preload',
  layer: 'platform',
  source: 'apps/desktop/src/renderer/bridge',
  contract: [
    'apps/desktop/src/renderer/bridge/mika-runtime-channel.ts',
    'apps/desktop/src/renderer/bridge/mika-runtime-chat.ts',
    'apps/desktop/src/renderer/bridge/mika-runtime-events.ts',
    'apps/desktop/src/renderer/bridge/mika-runtime-settings.ts',
    'apps/desktop/src/renderer/bridge/mika-runtime-system.ts',
  ],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
  frozen: { at: '2026-11-26', hash: '4af931c4023dba75' },
})
