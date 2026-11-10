/**
 * What the HUD may paint per `character.backgroundMode`.
 *
 * Transparent (the default) floats over someone else's game or desktop: at rest it keeps only the
 * small presence orb, everything else shows up while it is in use and leaves nothing behind — no
 * resting panels, no glows or shadows tinting the screen underneath. Semi-transparent allows light
 * glass but still hides; solid owns every pixel of its window and can show the full HUD.
 */
export type BackgroundMode = 'transparent' | 'semi-transparent' | 'solid'

export type HudPolicy = {
  density: 'minimal' | 'light' | 'full'
  /** Dock slides away when idle and comes back on hover/proximity/keyboard focus. */
  dockAutoHide: boolean
  dockSurface: 'glass' | 'solid'
  /** Radial glows around dock/bubble — they tint whatever is behind a see-through window. */
  ambientGlow: boolean
  /** Senses/mood/cost visible without hovering the presence orb. */
  telemetryAtRest: boolean
  /** Whether the presence orb shows its status word while nothing is happening. */
  presenceLabelAtRest: boolean
  /** Game session: a badge on the orb, a compact chip, or the full panel. */
  gameHud: 'badge' | 'chip' | 'panel'
}

const POLICIES: Record<BackgroundMode, HudPolicy> = {
  transparent: {
    density: 'minimal',
    dockAutoHide: true,
    dockSurface: 'glass',
    ambientGlow: false,
    telemetryAtRest: false,
    presenceLabelAtRest: false,
    gameHud: 'badge',
  },
  'semi-transparent': {
    density: 'light',
    dockAutoHide: true,
    dockSurface: 'glass',
    ambientGlow: false,
    telemetryAtRest: false,
    presenceLabelAtRest: false,
    gameHud: 'chip',
  },
  solid: {
    density: 'full',
    dockAutoHide: false,
    dockSurface: 'solid',
    ambientGlow: true,
    telemetryAtRest: true,
    presenceLabelAtRest: true,
    gameHud: 'panel',
  },
}

export function hudPolicy(mode: BackgroundMode): HudPolicy {
  return POLICIES[mode] ?? POLICIES.transparent
}
