import type { VrmBodyAction } from '@mika/sdk/contracts'

/**
 * Captured `.vrma` motions bundled with the app (credits in public/vrm/animations/CREDITS.md).
 * Wide-stance clips (dance, squat, spin, jump, show-full-body, greeting crouch) were left out: they
 * spread the legs through the pencil skirt.
 */
export const IDLE_CLIP = 'idle_loop'

/** Actions whose captured motion reads better than the procedural pose. */
export const VRM_ACTION_CLIPS: Partial<Record<VrmBodyAction, string>> = {
  surprised: 'Surprised',
  sigh: 'Sad',
  cheer: 'Clapping',
  wave: 'Goodbye',
  pout: 'Angry',
  point: 'shoot',
  shy: 'Blush',
  thinking: 'Thinking',
  peace: 'peaceSign',
  presenting: 'modelPose',
  stretch: 'Relax',
  curious: 'LookAround',
  yawn: 'Sleepy',
}

export const VRM_CLIP_URLS: Record<string, string> = Object.fromEntries(
  [IDLE_CLIP, ...Object.values(VRM_ACTION_CLIPS)].map((name) => [name, `/vrm/animations/${name}.vrma`]),
)
