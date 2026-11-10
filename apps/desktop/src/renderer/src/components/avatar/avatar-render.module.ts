import { defineModule } from '@mika/sdk/contracts'

// C3 — Live2D and VRM behind the same `AvatarEngine` contract; only proof is E2E with the PNG opened (action screenshots have photographed idle instead of the action 3x before).
export const AvatarRenderModule = defineModule({
  id: 'avatar-render',
  title: 'Renderizacao do avatar',
  layer: 'expression',
  source: 'apps/desktop/src/renderer/src/components/avatar',
  contract: [
    'apps/desktop/src/renderer/src/components/avatar/avatar-engine.ts',
    'apps/desktop/src/renderer/src/components/avatar/avatar-canvas.types.ts',
    'apps/desktop/src/renderer/src/components/avatar/vrm-adapter.ts',
    'apps/desktop/src/renderer/src/components/avatar/vrm-action-clock.ts',
    'apps/desktop/src/renderer/src/components/avatar/vrm-body-driver.ts',
    'apps/desktop/src/renderer/src/components/avatar/haru-lighting.ts',
    'apps/desktop/src/renderer/src/components/avatar/vrm-body-rig.ts',
    'apps/desktop/src/renderer/src/components/avatar/vrm-look.ts',
    'apps/desktop/src/renderer/src/components/avatar/vrm-body-poses.ts',
    'apps/desktop/src/renderer/src/components/avatar/vrm-pose.ts',
    'apps/desktop/src/renderer/src/components/avatar/vrm-scene-setup.ts',
  ],
  deps: [],
  consumes: [],
  emits: [],
  config: ['character.'],
  // Reopened for Haru HD: app homologation pending, do not reuse the previous certificate.
})
