import type { VTuberExpression } from '@mika/sdk/contracts'
import { invoke } from '@tauri-apps/api/core'
import { useEffect, useRef } from 'react'
// Reuses the desktop VRM engine as-is: the prototype measures whether it runs in Android WebView.
import { VrmAvatar } from '../../desktop/src/renderer/src/components/avatar/vrm-adapter'

interface AvatarViewProps {
  expression: VTuberExpression
  speaking: boolean
  onStatus: (status: string) => void
}

export function AvatarView({ expression, speaking, onStatus }: AvatarViewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const avatarRef = useRef<VrmAvatar | null>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const avatar = new VrmAvatar(canvas, window.innerWidth, window.innerHeight)
    avatarRef.current = avatar
    avatar.setQuality('reduced')
    avatar.setPostFx(true)
    let raf = 0
    let last = performance.now()
    const frame = (now: number) => {
      raf = requestAnimationFrame(frame)
      avatar.update(Math.min((now - last) / 1000, 0.1))
      last = now
      avatar.render()
    }
    const onResize = () => avatar.resize(window.innerWidth, window.innerHeight)
    window.addEventListener('resize', onResize)
    onStatus('Carregando avatar...')
    let modelUrl = ''
    invoke<ArrayBuffer>('avatar_model')
      .then((bytes) => {
        modelUrl = URL.createObjectURL(new Blob([bytes]))
        return avatar.load(modelUrl)
      })
      .then(() => onStatus('Avatar pronto'))
      .catch((error: unknown) => onStatus(`Falha no avatar: ${String(error)}`))
    raf = requestAnimationFrame(frame)
    return () => {
      cancelAnimationFrame(raf)
      window.removeEventListener('resize', onResize)
      avatar.dispose()
      if (modelUrl) URL.revokeObjectURL(modelUrl)
    }
  }, [onStatus])

  useEffect(() => avatarRef.current?.setExpression(expression), [expression])
  useEffect(() => avatarRef.current?.setSpeaking(speaking), [speaking])

  return <canvas ref={canvasRef} className="avatar" aria-label="Avatar da Mika" />
}
