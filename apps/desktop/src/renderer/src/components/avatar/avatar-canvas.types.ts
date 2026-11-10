import type { VTuberExpression } from '@mika/sdk/contracts'
import type { VowelWeights } from '../../lib/lip-sync-vowels'

/** Props do canvas de avatar, em UM lugar — `Live2DCanvas` e `VrmCanvas` são intercambiáveis por `App.tsx`. */
export interface AvatarCanvasProps {
  expression: VTuberExpression
  intensity?: number
  modelPath: string
  /** Ação nomeada a tocar quando o valor muda. `null` significa "nenhuma ação pendente". */
  action?: string | null
  volumeRef?: { current: number }
  vowelsRef?: { current: VowelWeights }
  mouseRef?: { current: { x: number; y: number } | undefined }
  isSpeaking?: boolean
  onAvatarInteract?: (x?: number, y?: number) => void
}
