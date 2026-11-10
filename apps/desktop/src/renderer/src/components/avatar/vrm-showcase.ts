import * as THREE from 'three'

// Stage dressing for the Skin screen's character select: 3-point studio lighting plus a soft contact
// shadow and a faint glow disc on the floor. Everything here is added to the avatar's own scene and
// removed again by the returned disposer, so switching skins leaves nothing behind.

const SHADOW_TEXTURE_SIZE = 64

/** Radial alpha falloff (opaque centre → transparent rim) for the blob shadow and the glow disc. */
function radialTexture(power: number): THREE.DataTexture {
  const size = SHADOW_TEXTURE_SIZE
  const data = new Uint8Array(size * size * 4)
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const r = Math.min(1, Math.hypot(x - size / 2 + 0.5, y - size / 2 + 0.5) / (size / 2))
      const i = (y * size + x) * 4
      data.fill(255, i, i + 3)
      data[i + 3] = Math.round(255 * (1 - r) ** power)
    }
  }
  const texture = new THREE.DataTexture(data, size, size, THREE.RGBAFormat)
  texture.needsUpdate = true
  return texture
}

function floorDisc(texture: THREE.Texture, color: THREE.ColorRepresentation, opacity: number) {
  const material = new THREE.MeshBasicMaterial({
    map: texture,
    color,
    transparent: true,
    opacity,
    depthWrite: false,
  })
  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(1, 1), material)
  mesh.rotation.x = -Math.PI / 2
  mesh.renderOrder = -1
  return mesh
}

// The avatar's own key light (vrm-scene-setup / haru-lighting) stays as the key; the stage adds the
// other two points: a cool low fill from the opposite side and a rim from behind, tinted by the theme.
function studioLights(accent: THREE.Color): THREE.Light[] {
  const fill = new THREE.DirectionalLight(0xdfe8ff, 0.25)
  fill.position.set(-2, 1.2, 1.6)
  const rim = new THREE.DirectionalLight(accent, 1.4)
  rim.position.set(-0.8, 2.2, -2.4)
  const kicker = new THREE.DirectionalLight(0xffffff, 0.6)
  kicker.position.set(1.4, 1.6, -2)
  return [fill, rim, kicker]
}

/** Studio look: less flat ambient so the key light actually models the form. Returns the undo. */
function dimAmbient(scene: THREE.Scene, factor: number): () => void {
  const ambients = scene.children.filter((o): o is THREE.AmbientLight => o instanceof THREE.AmbientLight)
  const before = ambients.map((light) => light.intensity)
  for (const light of ambients) light.intensity *= factor
  return () => {
    ambients.forEach((light, i) => {
      light.intensity = before[i] ?? light.intensity
    })
  }
}

/** Mounts lights and floor under a model standing on `floorY` with the given footprint width. */
export function mountShowcaseStage(
  scene: THREE.Scene,
  floorY: number,
  footprint: number,
  accent: THREE.ColorRepresentation,
): () => void {
  const accentColor = new THREE.Color(accent)
  const shadowTexture = radialTexture(1.6)
  const glowTexture = radialTexture(2.4)
  const shadow = floorDisc(shadowTexture, 0x000000, 0.7)
  shadow.scale.setScalar(Math.max(0.6, footprint * 1.1))
  shadow.position.y = floorY + 0.002
  // Normal blending on purpose: additive light on a transparent canvas writes no alpha and vanishes.
  const glow = floorDisc(glowTexture, accentColor, 0.45)
  glow.scale.setScalar(Math.max(1.6, footprint * 3.2))
  glow.position.y = floorY + 0.001
  const lights = studioLights(accentColor)
  const added: THREE.Object3D[] = [glow, shadow, ...lights]
  for (const object of added) scene.add(object)
  const restoreAmbient = dimAmbient(scene, 0.65)
  return () => {
    restoreAmbient()
    for (const object of added) scene.remove(object)
    for (const disc of [shadow, glow]) {
      disc.geometry.dispose()
      disc.material.dispose()
    }
    shadowTexture.dispose()
    glowTexture.dispose()
  }
}
