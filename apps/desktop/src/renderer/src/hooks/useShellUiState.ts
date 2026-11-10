import type { VTuberExpression } from '@mika/sdk/contracts'
import { useCallback, useRef, useState } from 'react'
import type { HealthCapabilities } from '../components/app/app-shell.types'
import { DEFAULT_CAPABILITIES } from '../components/app/app-shell.types'
import { DEFAULT_MODEL_PATH } from '../components/live2d/model-path'

/** Groups the shell's plain UI state (composer input, expression, avatar/background config) with no side effects. */
export function useShellUiState() {
  const [input, setInput] = useState('')
  const [expression, setExpressionState] = useState<VTuberExpression>('neutral')
  const [expressionIntensity, setExpressionIntensity] = useState(1.0)

  const setExpression = useCallback((expr: VTuberExpression, intensity = 1.0) => {
    setExpressionState(expr)
    setExpressionIntensity(intensity)
  }, [])

  const [live2dModelPath, setLive2dModelPath] = useState(DEFAULT_MODEL_PATH)
  const [backgroundMode, setBackgroundMode] = useState<'transparent' | 'semi-transparent' | 'solid'>('transparent')
  const [backgroundSolidColor, setBackgroundSolidColor] = useState('#050914')
  const [capabilities, setCapabilities] = useState<HealthCapabilities>(DEFAULT_CAPABILITIES)
  const [mikaReady, setMikaReady] = useState(false)

  const volumeRef = useRef(0)
  const ttsVolumeRef = useRef(1)
  const mousePosRef = useRef<{ x: number; y: number } | undefined>(undefined)

  return {
    input,
    setInput,
    expression,
    expressionIntensity,
    setExpression,
    live2dModelPath,
    setLive2dModelPath,
    backgroundMode,
    setBackgroundMode,
    backgroundSolidColor,
    setBackgroundSolidColor,
    capabilities,
    setCapabilities,
    mikaReady,
    setMikaReady,
    volumeRef,
    ttsVolumeRef,
    mousePosRef,
  }
}
