import { defineModule } from '@mika/sdk/contracts'

// C5 — presence orb, speech bubble, game chip, mood HUD, dock and toasts, laid out around her face (components/hud/); PanelLayer.tsx touches `window` and only loads with a DOM, so UI proof is the opened E2E PNG.
// ChatFeedOverlay removed 2026-09-24: dead feature, no Rust producer ever published on `channel:message` (see hud_bridge.rs test).
export const HudOverlayModule = defineModule({
  id: 'hud-overlay',
  title: 'HUD e overlay',
  layer: 'expression',
  source: 'apps/desktop/src/renderer/src/components/app',
  contract: [
    'apps/desktop/src/renderer/src/components/app/HudLayer.tsx',
    'apps/desktop/src/renderer/src/components/app/PanelLayer.tsx',
    'apps/desktop/src/renderer/src/components/app/ChatComposer.tsx',
    'apps/desktop/src/renderer/src/components/app/AvatarStageBoundary.tsx',
    'apps/desktop/src/renderer/src/components/app/app-shell.types.ts',
  ],
  deps: [],
  consumes: [],
  emits: [],
  config: [],
  frozen: { at: '2026-09-28', hash: '342e3445d9948910' },
})
