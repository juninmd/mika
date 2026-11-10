import * as THREE from 'three'

// No tone mapping: ACES desaturates the MToon palette and washes out the outline.
export function mountHaruLighting(scene: THREE.Scene, renderer: THREE.WebGLRenderer): () => void {
  const originalLights = scene.children.filter((object) => object instanceof THREE.Light)
  for (const light of originalLights) light.visible = false
  const group = new THREE.Group()
  group.name = 'HaruStudioLighting'
  group.add(new THREE.AmbientLight(0xfff6f2, 0.55))
  const key = new THREE.DirectionalLight(0xfff4ec, 2.1)
  key.position.set(-1.4, 2.4, 3)
  const rim = new THREE.DirectionalLight(0xd8ccff, 0.35)
  rim.position.set(1.5, 2, -2.5)
  group.add(key, rim)
  scene.add(group)
  const toneMapping = renderer.toneMapping
  renderer.toneMapping = THREE.NoToneMapping
  return () => {
    scene.remove(group)
    for (const light of originalLights) light.visible = true
    renderer.toneMapping = toneMapping
  }
}
