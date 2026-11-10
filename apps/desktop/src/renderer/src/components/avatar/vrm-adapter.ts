import { clamp01 } from '@mika/sdk/utils/clamp'
import { type VRM, VRMUtils } from '@pixiv/three-vrm'
import * as THREE from 'three'
import type { EffectQuality } from '../../lib/effect-quality'
import { currentRenderPixelRatio } from '../../lib/render-pixel-ratio'
import { rendererLog } from '../../lib/renderer-logger'
import { useEffectsStore } from '../../store/useEffectsStore'
import type { AvatarEngine, AvatarEngineCapabilities, NumberRef, VowelsRef } from './avatar-engine'
import { mountHaruLighting } from './haru-lighting'
import {
  ACCESSORIES_BY_LOOK,
  type MountedAccessory,
  mountAccessories,
  syncAccessories,
  unmountAccessories,
} from './vrm-accessories'
import { VrmActionClock } from './vrm-action-clock'
import { BlinkDriver } from './vrm-blink'
import { blushTarget, VrmBlush } from './vrm-blush'
import { VrmBodyDriver } from './vrm-body-driver'
import type { ArmPose } from './vrm-body-rig'
import type { VrmLifeSnapshot } from './vrm-debug-bridge'
import { applyFabricDetail } from './vrm-fabric'
import { IDLE_STATES, type IdleState } from './vrm-idle-states'
import { VrmKatana } from './vrm-katana'
import { bodyActionForExpression, expressionPreset, ORIGINAL_LOOK, type VrmLook } from './vrm-look'
import { applyOutlineInk, sharpenTextures, tuneAnimeMaterials, worldPerPixel } from './vrm-materials'
import { VisemeSmoother, type VowelWeightsLike, visemeTargets } from './vrm-mouth'
import { AnimePostFx } from './vrm-postfx'
import { VrmActionProps } from './vrm-props'
import {
  addDefaultLights,
  applyLookTints,
  blendPresets,
  disposeVrmScene,
  fitCamera,
  loadVrmGltf,
  readVrmLife,
  stepVrm,
  writeBlink,
  writeVisemes,
} from './vrm-scene-setup'
import { applyHairShine } from './vrm-shine'
import { DEFAULT_WIND_STRENGTH, SpringWind, type Vec3, windInto } from './vrm-wind'

export type { VowelWeightsLike } from './vrm-mouth'

// Same AvatarEngine contract as the Live2D animator, backed by a VRM model via three.js + @pixiv/three-vrm.

export class VrmAvatar implements AvatarEngine {
  readonly capabilities: AvatarEngineCapabilities = { bodyRig: true, namedActions: true }

  private volumeRef?: NumberRef
  private vowelsRef?: VowelsRef

  readonly scene = new THREE.Scene()
  readonly camera: THREE.PerspectiveCamera
  private readonly renderer: THREE.WebGLRenderer
  private vrm: VRM | null = null
  private mouthOpen = 0
  private vowels: VowelWeightsLike = { a: 0, i: 0, u: 0, e: 0, o: 0 }
  private expression = 'neutral'
  /** Raw name: `shy` and `love` both collapse to the `happy` preset, but only they blush. */
  private expressionName = 'neutral'
  private expressionIntensity = 0
  private speaking = false
  private readonly action = new VrmActionClock()
  private readonly body: VrmBodyDriver
  private readonly props: VrmActionProps
  private readonly katana: VrmKatana
  private readonly blink = new BlinkDriver()
  private readonly blush = new VrmBlush()
  private readonly mouth = new VisemeSmoother()
  private readonly presetVelocity: Record<string, number> = {}
  private wind?: SpringWind
  private readonly windNow: Vec3 = { x: 0, y: 0, z: 0 }
  private windStrength = DEFAULT_WIND_STRENGTH
  private postFx?: AnimePostFx
  private postFxEnabled = true
  private idleEnabled = true
  private accessories: MountedAccessory[] = []
  private elapsed = 0
  private releaseHaruLighting?: () => void
  private width = 1
  private height = 1
  private disposed = false

  constructor(canvas: HTMLCanvasElement, width: number, height: number) {
    // Same as the Live2D stage: without it a hybrid laptop renders the avatar on the iGPU.
    this.renderer = new THREE.WebGLRenderer({
      canvas,
      // O post-FX desenha a cena num alvo MSAA próprio e só um quad chega ao canvas: MSAA no canvas
      // era um segundo resolve por quadro sem efeito visível.
      antialias: false,
      alpha: true,
      powerPreference: 'high-performance',
    })
    // WebGLState leaves the GL clear color at (0,0,0,1); RenderPass clears before three pushes alpha 0,
    // so the first post-processed frame came out as an opaque black background.
    this.renderer.setClearColor(0x000000, 0)
    // Cor fiel às texturas do autor: saída sRGB e sem tone mapping — o ACES/Filmic "fotográfico" achata e
    // dessatura a paleta de anime. É o padrão do three; fica explícito para ninguém ligar sem perceber.
    this.renderer.outputColorSpace = THREE.SRGBColorSpace
    this.renderer.toneMapping = THREE.NoToneMapping
    this.camera = new THREE.PerspectiveCamera(28, 1, 0.1, 20)
    this.resize(width, height)
    addDefaultLights(this.scene)
    this.body = new VrmBodyDriver(this.scene)
    this.props = new VrmActionProps(this.scene, this.camera)
    // The blade's petals open the same full-screen /sakura storm the chat command does.
    this.katana = new VrmKatana(this.scene, () =>
      useEffectsStore.getState().applyMatchEffect({ effect: 'sakura', mode: 'show' }),
    )
    this.postFx = new AnimePostFx(this.renderer, this.scene, this.camera)
  }

  resize(width: number, height: number): void {
    this.width = width
    this.height = height
    // Reaplicado a cada resize: a janela pode ter mudado para um monitor com outra densidade.
    this.renderer.setPixelRatio(currentRenderPixelRatio())
    this.renderer.setSize(width, height, false)
    this.postFx?.syncSize()
    fitCamera(this.camera, width, height, this.vrm)
    this.refreshInk()
  }

  /** O traço tem piso em pixels de tela: recalcula quando o enquadramento ou a janela mudam. */
  private refreshInk(): void {
    const frame = this.modelFrame()
    if (!frame || !this.vrm) return
    const distance = this.camera.position.distanceTo(frame.bounds.getCenter(new THREE.Vector3()))
    applyOutlineInk(this.vrm.scene, worldPerPixel(distance, this.camera.fov, this.height))
  }

  async load(url: string, look: VrmLook = ORIGINAL_LOOK): Promise<void> {
    const { gltf, vrm } = await loadVrmGltf(url)
    if (this.disposed) {
      // The stage went away while the model downloaded: nothing will ever own it, free it.
      VRMUtils.deepDispose(vrm.scene)
      return
    }
    this.vrm = vrm
    this.releaseHaruLighting?.()
    this.releaseHaruLighting = undefined
    if (gltf.scene.getObjectsByProperty('name', 'Haru_Blazer').length > 0) {
      this.releaseHaruLighting = mountHaruLighting(this.scene, this.renderer)
    }
    tuneAnimeMaterials(vrm.scene)
    applyFabricDetail(vrm.scene)
    applyHairShine(vrm.scene)
    sharpenTextures(vrm.scene, Math.min(8, this.renderer.capabilities.getMaxAnisotropy()))
    this.wind?.restore()
    this.wind = vrm.springBoneManager ? new SpringWind(vrm.springBoneManager.joints) : undefined
    this.body.attach(vrm)
    this.props.attach(vrm)
    this.katana.attach(vrm)
    await this.body.loadClips(vrm).catch((error: unknown) => {
      rendererLog.warn('[VrmAvatar] clipes .vrma indisponíveis, só o rig procedural', { error: String(error) })
    })
    // dispose() ran during the clip load: it already freed `this.vrm`, so don't re-attach it to a dead scene.
    if (this.disposed) return
    this.blush.attach(vrm)
    this.applyLook(look)
    vrm.update(0)
    // Accessories spawn at the origin; unsynced, framing would measure Mao's hat at her feet.
    syncAccessories(this.accessories)
    this.scene.add(vrm.scene)
    this.postFx?.trackGlow(vrm.scene)
    // Bones only exist to frame against once the model is in the scene — refit now instead of
    // waiting for the next window resize.
    fitCamera(this.camera, this.width, this.height, vrm)
    this.refreshInk()
  }

  /** World bounds of the loaded model alone (no props, lights or stage dressing) plus its head
   * position, for showcase framing; null before load. */
  modelFrame(): { bounds: THREE.Box3; head: THREE.Vector3 } | null {
    if (!this.vrm) return null
    this.vrm.scene.updateMatrixWorld(true)
    const bounds = new THREE.Box3().setFromObject(this.vrm.scene)
    const headNode = this.vrm.humanoid?.getRawBoneNode('head')
    const head = headNode ? headNode.getWorldPosition(new THREE.Vector3()) : bounds.getCenter(new THREE.Vector3())
    return { bounds, head }
  }

  applyLook(look: VrmLook): void {
    if (this.vrm) this.accessories = mountAccessories(this.vrm, ACCESSORIES_BY_LOOK[look.id] ?? [])
    applyLookTints(this.vrm?.scene, look)
  }

  setLipSyncSources(volume?: NumberRef, vowels?: VowelsRef): void {
    this.volumeRef = volume
    this.vowelsRef = vowels
  }

  /** E2E-only: writes straight into the same refs the audio analyser drives, to prove the
   * volume/vowels → viseme → blend-shape path end to end without real TTS audio (off in E2E). */
  driveLipSyncForTest(volume: number, vowels: VowelWeightsLike): void {
    if (this.volumeRef) this.volumeRef.current = volume
    if (this.vowelsRef) this.vowelsRef.current = vowels
  }

  reactToPoke(): void {
    const roll = Math.random()
    if (roll < 0.35) this.body.nod()
    else if (roll < 0.7) this.body.jump()
    else this.playAction('shy')
  }

  // Distinct from reactToPoke(): always the same pose, never a random one. 'shy' already gives a
  // small head-tilt (vrm-body-poses.ts POSES.shy: headX/headZ) plus a happy-preset expression whose
  // intensity closes the eyes (see EYES_BUSY_THRESHOLD in vrm-blink.ts).
  reactToHeadPat(): void {
    this.setExpression('shy', 1)
  }

  setExpression(name: string, intensity = 1): void {
    this.expression = expressionPreset(name)
    this.expressionName = name
    this.expressionIntensity = clamp01(intensity, 1)
    if (intensity < 0.5) return
    const action = bodyActionForExpression(name)
    if (!action) return
    if (this.action.remaining <= 0) {
      this.playAction(action)
    } else {
      // Tool already told the LLM it succeeded; without this the miss looks like a flaky no-op.
      rendererLog.debug('[VrmAvatar] pose descartada, ação em andamento', {
        action,
        remaining: this.action.remaining,
      })
    }
  }

  setSpeaking(speaking: boolean): void {
    this.speaking = speaking
  }

  playAction(name: string): void {
    if (this.action.start(name)) this.body.play(name)
    else rendererLog.warn('[VrmAvatar] ação recusada, nome desconhecido', { name })
  }

  setCursorTarget(x: number, y: number): void {
    this.body.setCursorTarget(x, y)
  }

  /** Plano de render (supersampling/MSAA) e bloom acompanham a qualidade: foco, bateria, jogo. */
  setQuality(quality: EffectQuality): void {
    this.postFx?.setQuality(quality)
  }

  setPostFx(enabled: boolean): void {
    this.postFxEnabled = enabled
  }

  /** Reduced motion: moves still play, but effects (the senbonzakura petal storm) shrink to a hint. */
  setReducedMotion(reduced: boolean): void {
    this.katana.setReducedMotion(reduced)
  }

  /** 0..1 (`windStrengthFromConfig`): wind in the hair, skirt and ribbons, and on the petals. */
  setWind(strength: number): void {
    this.windStrength = strength
  }

  /** Debug bridge: holds one of the long idle states now (it still yields to speech and gestures). */
  playIdleState(name: string): boolean {
    if (!(IDLE_STATES as readonly string[]).includes(name)) return false
    this.body.playIdleState(name as IdleState)
    return true
  }

  /** Skin stage: long idle states show up within seconds instead of after a quiet spell. */
  setShowcase(showcase: boolean): void {
    this.body.setShowcase(showcase)
  }

  /** Backgrounded/reduced-motion windows keep breathing sway but skip autonomous idle poses. */
  setIdleEnabled(enabled: boolean): void {
    this.idleEnabled = enabled
  }

  update(dt: number): void {
    const vrm = this.vrm
    if (!vrm) return
    dt = Number.isFinite(dt) ? Math.max(0, Math.min(dt, 0.05)) : 0
    // Lip-sync sources are pulled here, not pushed per frame by the component, so both backends share one ref.
    if (this.volumeRef) this.mouthOpen = clamp01(this.volumeRef.current, 0)
    if (this.vowelsRef) this.vowels = this.vowelsRef.current
    this.elapsed += dt
    this.updateExpressions(vrm, dt)
    this.body.update(vrm, dt, this.elapsed, this.action, this.speaking, this.mouthOpen, this.idleEnabled)
    // The grip bends the arms onto the hilt on the normalized rig, so it must land before the copy.
    this.katana.hold(this.action.name, this.action.progress)
    windInto(this.windNow, this.elapsed, this.windStrength)
    this.wind?.apply(this.windNow)
    stepVrm(vrm, dt)
    syncAccessories(this.accessories)
    this.props.update(dt, this.action.name, this.action.progress)
    this.katana.update(dt, this.action.name, this.action.progress, this.windNow)
    this.blush.update(blushTarget(this.expressionName, this.action.name, this.action.pulse), dt)
  }

  private updateExpressions(vrm: VRM, dt: number): void {
    const em = vrm.expressionManager
    if (!em) return
    const action = this.action
    action.tick(dt)
    const pulse = action.pulse
    blendPresets(
      em,
      this.expression,
      this.expressionIntensity,
      action.preset,
      action.name,
      pulse,
      dt,
      this.presetVelocity,
    )
    writeVisemes(em, this.mouth.advance(visemeTargets(this.vowels, this.mouthOpen, this.speaking), dt))
    writeBlink(em, this.blink, action.isPlaying('wink'), dt, this.body.gazeSpeed, this.speaking)
    em.setValue('blinkLeft', action.name === 'wink' ? pulse : 0)
  }

  readArmRotations(): ArmPose {
    return this.body.readArmRotations(this.vrm)
  }

  // Debug bridge read: lets E2E prove gaze, blink, mouth and spring physics in the real app.
  readLife(): VrmLifeSnapshot {
    return readVrmLife(this.vrm, this.renderer, this.postFxEnabled)
  }

  render(): void {
    if (this.postFxEnabled && this.postFx) this.postFx.render()
    else this.renderer.render(this.scene, this.camera)
  }

  dispose(): void {
    this.disposed = true
    this.wind?.restore()
    this.wind = undefined
    this.postFx?.dispose()
    this.postFx = undefined
    this.releaseHaruLighting?.()
    this.releaseHaruLighting = undefined
    this.blush.dispose()
    this.props.dispose()
    this.katana.dispose()
    if (this.vrm) {
      unmountAccessories(this.vrm)
      this.accessories = []
      disposeVrmScene(this.scene, this.vrm)
      this.vrm = null
    }
    this.renderer.dispose()
  }
}
