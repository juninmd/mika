import type { CSSProperties } from 'react'

export function layerStyle(layer: string, extra?: CSSProperties): CSSProperties {
  return { zIndex: `var(--layer-${layer})`, ...extra }
}
