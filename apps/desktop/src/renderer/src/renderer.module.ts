import { defineModule } from '@mika/sdk/contracts'

export const RendererModule = defineModule({
  id: 'renderer',
  title: 'Interface (renderer)',
  layer: 'product',
  source: 'apps/desktop/src/renderer',
  contract: [
    'apps/desktop/src/renderer/src/themes.ts',
    'apps/desktop/src/renderer/src/store/useChatStore.ts',
    'apps/desktop/src/renderer/src/store/useUIStore.ts',
    'apps/desktop/src/renderer/src/store/useRegistryStore.ts',
    'apps/desktop/src/renderer/src/store/useToolExecutionStore.ts',
    'apps/desktop/src/renderer/src/store/useEffectsStore.ts',
    'apps/desktop/src/renderer/src/store/useTrackingStore.ts',
  ],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
  frozen: { at: '2026-10-06', hash: 'a8f6f2d1af056705' },
})
