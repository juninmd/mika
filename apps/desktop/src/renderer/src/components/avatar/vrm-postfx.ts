/** Acabamento de tela do VRM: um passe só com nitidez, brilho difuso, tons divididos e saturação. */
import * as THREE from 'three'
import { FullScreenQuad } from 'three/examples/jsm/postprocessing/Pass.js'
import type { EffectQuality } from '../../lib/effect-quality'

export const ANIME_GRADE = {
  sharpen: 0.35,
  /** Força do bloom seletivo (só as áreas emissivas); o antigo brilho por luminância lavava pele e tecido claro. */
  glow: 0.55,
  split: 0.1,
  saturation: 1.08,
  shadowTint: new THREE.Color('#5c5a9a'),
  lightTint: new THREE.Color('#ffe6d6'),
}

/**
 * Um passe só: cada amostra sai do alvo linear e vira cor de saída (sRGB) já no `tap`, onde "sombra" e
 * "luz" têm o sentido do olho. Antes um `OutputPass` fazia essa conversão num passe de tela cheia à
 * parte, escrevendo num segundo alvo MSAA HalfFloat só para o grade ler de volta.
 */
/**
 * Plano de render: quanto supersampling (o alvo da cena em N× a resolução da tela, reduzido pelo
 * filtro bilinear do passe final) e quantas amostras de MSAA. Supersampling pega o que o MSAA não
 * pega — o traço fino de tinta e o serrilhado de textura —, mas custa N² em pixels, então só vale em
 * tela de densidade 1 e com a memória do alvo dentro do orçamento.
 */
export interface RenderPlan {
  supersample: number
  samples: number
}

/** Orçamento de memória dos alvos da cena (MSAA + resolve, RGBA16F): ~192 MB. */
export const TARGET_BUDGET_BYTES = 192 * 1024 * 1024
const BYTES_PER_PIXEL = 8

/** Do melhor ao mais barato; o primeiro que cabe no orçamento e na GPU vence. */
const FULL_PLANS: readonly RenderPlan[] = [
  { supersample: 2, samples: 4 },
  { supersample: 2, samples: 2 },
  { supersample: 2, samples: 0 },
  { supersample: 1, samples: 8 },
  { supersample: 1, samples: 4 },
  { supersample: 1, samples: 2 },
  { supersample: 1, samples: 0 },
]

export function targetBytes(cssWidth: number, cssHeight: number, plan: RenderPlan, pixelRatio = 1): number {
  const pixels = cssWidth * pixelRatio * cssHeight * pixelRatio * plan.supersample ** 2
  return pixels * BYTES_PER_PIXEL * (plan.samples > 0 ? plan.samples + 1 : 1)
}

export function planFor(
  quality: EffectQuality,
  cssWidth: number,
  cssHeight: number,
  pixelRatio: number,
  maxSamples: number,
): RenderPlan {
  const ceiling = quality === 'full' ? 8 : quality === 'reduced' ? 4 : 2
  const dense = pixelRatio >= 2
  const candidates = quality === 'full' ? FULL_PLANS : FULL_PLANS.filter((plan) => plan.supersample === 1)
  const fits = (plan: RenderPlan) =>
    plan.samples <= Math.min(ceiling, maxSamples) &&
    !(dense && plan.supersample > 1) &&
    targetBytes(cssWidth, cssHeight, plan, pixelRatio) <= TARGET_BUDGET_BYTES
  return candidates.find(fits) ?? { supersample: 1, samples: 0 }
}

/**
 * Bloom só no que emite luz: olhos (íris e brilho) e os detalhes luminosos da roupa. Pele, cabelo e
 * tecido liso ficam de fora — o brilho por luminância que havia antes espalhava luz sobre todo o claro.
 */
const GLOW_MATERIAL = /EyeIris|EyeHighlight|Cuff_Rib|arcade_Hem|SockStripe|SignalMint|Gamepad|PixelHeart/i

export function isGlowMaterial(name: string): boolean {
  return GLOW_MATERIAL.test(name)
}

const AnimeGradeShader = {
  name: 'MikaAnimeGrade',
  uniforms: {
    tDiffuse: { value: null },
    tGlow: { value: null },
    texel: { value: new THREE.Vector2(1, 1) },
    sharpen: { value: ANIME_GRADE.sharpen },
    glow: { value: ANIME_GRADE.glow },
    split: { value: ANIME_GRADE.split },
    saturation: { value: ANIME_GRADE.saturation },
    shadowTint: { value: ANIME_GRADE.shadowTint },
    lightTint: { value: ANIME_GRADE.lightTint },
  },
  vertexShader: /* glsl */ `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }`,
  fragmentShader: /* glsl */ `
    uniform sampler2D tDiffuse, tGlow;
    uniform vec2 texel;
    uniform float sharpen, glow, split, saturation;
    uniform vec3 shadowTint, lightTint;
    varying vec2 vUv;
    float luma(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }
    vec3 tap(vec2 offset) { return linearToOutputTexel(texture2D(tDiffuse, vUv + offset * texel)).rgb; }
    vec3 glowTap(vec2 offset) { return linearToOutputTexel(texture2D(tGlow, vUv + offset * texel)).rgb; }
    void main() {
      vec4 src = linearToOutputTexel(texture2D(tDiffuse, vUv));
      vec3 c = src.rgb;
      vec3 n = tap(vec2(0.0, 1.0)), s = tap(vec2(0.0, -1.0)), e = tap(vec2(1.0, 0.0)), w = tap(vec2(-1.0, 0.0));
      // Limite inclui o próprio pixel: um traço de 1 px mais escuro que os vizinhos não pode sumir.
      vec3 lo = min(c, min(min(n, s), min(e, w)));
      vec3 hi = max(c, max(max(n, s), max(e, w)));
      c = clamp(c + (c - (n + s + e + w) * 0.25) * sharpen, lo, hi);
      // Bloom seletivo: tGlow só tem as peças emissivas (olhos, detalhes luminosos), em meia resolução;
      // o halo é o próprio filtro bilinear mais um leque de amostras, e entra por screen.
      vec3 halo = glowTap(vec2(0.0)) * 0.28
        + (glowTap(vec2(3.0, 0.0)) + glowTap(vec2(-3.0, 0.0)) + glowTap(vec2(0.0, 3.0)) + glowTap(vec2(0.0, -3.0))) * 0.15
        + (glowTap(vec2(2.2, 2.2)) + glowTap(vec2(-2.2, 2.2)) + glowTap(vec2(2.2, -2.2)) + glowTap(vec2(-2.2, -2.2))) * 0.03;
      c = 1.0 - (1.0 - c) * (1.0 - clamp(halo * glow, 0.0, 1.0));
      float l = luma(c);
      c = mix(c, c * shadowTint * 2.0, clamp((0.5 - l) * 2.0, 0.0, 1.0) * split);
      c = mix(c, c * lightTint, clamp((l - 0.5) * 2.0, 0.0, 1.0) * split);
      c = mix(vec3(luma(c)), c, saturation);
      // Cor pré-multiplicada: passar do alfa acende halo sobre o desktop transparente.
      gl_FragColor = vec4(min(max(c, 0.0), vec3(src.a)), src.a);
    }`,
}

export class AnimePostFx {
  private target: THREE.WebGLRenderTarget
  private readonly glowTarget = new THREE.WebGLRenderTarget(1, 1, { type: THREE.HalfFloatType })
  private readonly material = new THREE.ShaderMaterial({
    ...AnimeGradeShader,
    uniforms: THREE.UniformsUtils.clone(AnimeGradeShader.uniforms),
  })
  private readonly quad = new FullScreenQuad(this.material)
  private readonly size = new THREE.Vector2()
  private plan: RenderPlan = { supersample: 1, samples: 4 }
  private quality: EffectQuality = 'full'
  private glowMeshes: THREE.Mesh[] = []
  private readonly occluded: { mesh: THREE.Mesh; material: THREE.Material | THREE.Material[] }[] = []
  private readonly occluder = new THREE.MeshBasicMaterial({ color: 0x000000 })

  constructor(
    private readonly renderer: THREE.WebGLRenderer,
    private readonly scene: THREE.Scene,
    private readonly camera: THREE.Camera,
  ) {
    // O canvas não tem MSAA; o alvo da cena tem o seu, senão o contorno serrilha.
    this.target = this.makeTarget(this.plan)
    this.material.uniforms.tDiffuse.value = this.target.texture
    this.material.uniforms.tGlow.value = this.glowTarget.texture
    this.syncSize()
  }

  private makeTarget(plan: RenderPlan): THREE.WebGLRenderTarget {
    return new THREE.WebGLRenderTarget(1, 1, { type: THREE.HalfFloatType, samples: plan.samples })
  }

  /** Reaplica o plano quando a qualidade muda (foco, bateria, jogo); só recria o alvo se o plano mudou. */
  setQuality(quality: EffectQuality): void {
    if (quality === this.quality) return
    this.quality = quality
    this.syncSize()
  }

  /** Lê as peças emissivas da cena: chamar depois de carregar o modelo (e de trocar o visual). */
  trackGlow(root: THREE.Object3D): void {
    const meshes: THREE.Mesh[] = []
    root.traverse((object) => {
      const mesh = object as THREE.Mesh
      if (!mesh.isMesh) return
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
      if (materials.some((material) => isGlowMaterial(material.name))) meshes.push(mesh)
    })
    this.glowMeshes = meshes
  }

  syncSize(): void {
    const buffer = this.renderer.getDrawingBufferSize(this.size)
    const ratio = this.renderer.getPixelRatio()
    const plan = planFor(this.quality, buffer.x / ratio, buffer.y / ratio, ratio, this.renderer.capabilities.maxSamples)
    if (plan.samples !== this.plan.samples) {
      this.target.dispose()
      this.target = this.makeTarget(plan)
      this.material.uniforms.tDiffuse.value = this.target.texture
    }
    this.plan = plan
    this.target.setSize(Math.max(1, buffer.x * plan.supersample), Math.max(1, buffer.y * plan.supersample))
    this.glowTarget.setSize(Math.max(1, Math.ceil(buffer.x / 2)), Math.max(1, Math.ceil(buffer.y / 2)))
    // `texel` é o pixel da TELA: o filtro do passe final reduz o alvo (maior) por interpolação bilinear.
    this.material.uniforms.texel.value.set(1 / Math.max(1, buffer.x), 1 / Math.max(1, buffer.y))
  }

  /**
   * Cena só com as peças emissivas, em meia resolução: o que o bloom enxerga. O resto vira oclusor preto
   * (não some): sem profundidade, a faixa menta escondida atrás do cabelo brilhava POR CIMA dele.
   */
  private renderGlow(): boolean {
    const visibleGlow = this.glowMeshes.filter((mesh) => mesh.visible)
    if (visibleGlow.length === 0) return false
    const glow = new Set<THREE.Object3D>(visibleGlow)
    this.occluded.length = 0
    this.scene.traverse((object) => {
      const mesh = object as THREE.Mesh
      if (!mesh.isMesh || !mesh.visible || glow.has(mesh)) return
      this.occluded.push({ mesh, material: mesh.material })
      mesh.material = this.occluder
    })
    this.renderer.setRenderTarget(this.glowTarget)
    this.renderer.clear()
    this.renderer.render(this.scene, this.camera)
    for (const { mesh, material } of this.occluded) mesh.material = material
    return true
  }

  render(): void {
    const previous = this.renderer.getRenderTarget()
    this.material.uniforms.glow.value = this.renderGlow() ? ANIME_GRADE.glow : 0
    this.renderer.setRenderTarget(this.target)
    this.renderer.render(this.scene, this.camera)
    this.renderer.setRenderTarget(previous)
    this.quad.render(this.renderer)
  }

  dispose(): void {
    this.target.dispose()
    this.glowTarget.dispose()
    this.occluder.dispose()
    this.material.dispose()
    this.quad.dispose()
  }
}
