import type { VRM } from '@pixiv/three-vrm'
import * as THREE from 'three'

// The VRM ships no blush shape, so the anime cheek flush is a painted decal that follows the head.
const BLUSH_EXPRESSIONS = new Set(['shy', 'embarrassed', 'love'])
const BLUSH_ACTIONS = new Set(['shy', 'heart'])
const HELD_BLUSH = 0.8
// Faint pink the Live2D Haru keeps at rest; the strokes only read once the flush rises.
export const RESTING_BLUSH = 0.15

export function blushTarget(expression: string, action: string, pulse: number): number {
  const fromAction = BLUSH_ACTIONS.has(action) && Number.isFinite(pulse) ? Math.min(1, Math.max(0, pulse)) : 0
  return Math.max(BLUSH_EXPRESSIONS.has(expression) ? HELD_BLUSH : RESTING_BLUSH, fromAction)
}

function paintBlush(): THREE.CanvasTexture {
  const canvas = document.createElement('canvas')
  canvas.width = 256
  canvas.height = 64
  const ctx = canvas.getContext('2d')
  if (ctx) {
    for (const cx of [52, 204]) {
      const glow = ctx.createRadialGradient(cx, 32, 2, cx, 32, 40)
      glow.addColorStop(0, 'rgba(255, 105, 135, 0.85)')
      glow.addColorStop(1, 'rgba(255, 105, 135, 0)')
      ctx.save()
      ctx.translate(cx, 32)
      ctx.scale(1, 0.5)
      ctx.translate(-cx, -32)
      ctx.fillStyle = glow
      ctx.fillRect(cx - 44, 0, 88, 64)
      ctx.restore()
      // The three short diagonal strokes anime draws over a flushed cheek.
      ctx.strokeStyle = 'rgba(214, 60, 96, 0.7)'
      ctx.lineWidth = 3
      ctx.lineCap = 'round'
      for (const dx of [-14, 0, 14]) {
        ctx.beginPath()
        ctx.moveTo(cx + dx - 4, 40)
        ctx.lineTo(cx + dx + 4, 24)
        ctx.stroke()
      }
    }
  }
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  return texture
}

const eyeWorld = new THREE.Vector3()
const worldPosition = new THREE.Vector3()
const worldQuaternion = new THREE.Quaternion()

export class VrmBlush {
  private group?: THREE.Group
  private material?: THREE.MeshBasicMaterial
  private head?: THREE.Object3D
  private level = 0

  /** Places the decal just under the eyes, measured on the rest pose before anything animates. */
  attach(vrm: VRM): void {
    this.dispose()
    const head = vrm.humanoid?.getNormalizedBoneNode('head')
    const left = vrm.humanoid?.getNormalizedBoneNode('leftEye')
    const right = vrm.humanoid?.getNormalizedBoneNode('rightEye')
    if (!head || !left || !right) return
    vrm.scene.updateMatrixWorld(true)
    const l = head.worldToLocal(left.getWorldPosition(eyeWorld).clone())
    const r = head.worldToLocal(right.getWorldPosition(eyeWorld).clone())
    // Eye bones pivot inside the skull, about half the visible eye spacing apart.
    const span = Math.max(0.05, l.distanceTo(r) * 2)
    this.material = new THREE.MeshBasicMaterial({
      map: paintBlush(),
      transparent: true,
      opacity: 0,
      depthWrite: false,
      toneMapped: false,
    })
    const plane = new THREE.Mesh(new THREE.PlaneGeometry(span * 1.8, span * 0.45), this.material)
    plane.position.set((l.x + r.x) / 2, (l.y + r.y) / 2 - span * 0.5, Math.max(l.z, r.z) + span * 1.05)
    plane.renderOrder = 2
    this.group = new THREE.Group()
    this.group.add(plane)
    this.group.visible = false
    vrm.scene.add(this.group)
    this.head = head
  }

  update(target: number, dt: number): void {
    if (!this.group || !this.material || !this.head) return
    this.level += (target - this.level) * (1 - Math.exp(-dt * 5))
    this.material.opacity = this.level
    this.group.visible = this.level > 0.01
    this.head.getWorldPosition(worldPosition)
    this.head.getWorldQuaternion(worldQuaternion)
    this.group.position.copy(worldPosition)
    this.group.quaternion.copy(worldQuaternion)
  }

  dispose(): void {
    const plane = this.group?.children[0] as THREE.Mesh | undefined
    plane?.geometry.dispose()
    this.material?.map?.dispose()
    this.material?.dispose()
    this.group?.parent?.remove(this.group)
    this.group = undefined
    this.material = undefined
    this.head = undefined
    this.level = 0
  }
}
