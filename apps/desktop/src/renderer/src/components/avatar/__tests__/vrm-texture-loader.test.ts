import * as THREE from 'three'
import { describe, expect, it } from 'vitest'
import { forceImgTextureLoader, type TextureLoaderHost } from '../vrm-texture-loader'

/** Em `file://` o Chromium recusa `fetch('blob:file:///…')`; `TextureLoader` via `<img>` funciona. */
describe('carregador de textura do VRM', () => {
  it('troca o carregador que usa fetch por um que usa <img>', () => {
    const parser: TextureLoaderHost = { textureLoader: new THREE.ImageBitmapLoader() }

    forceImgTextureLoader(parser)

    expect((parser.textureLoader as { isImageBitmapLoader?: boolean }).isImageBitmapLoader).toBeUndefined()
    expect(parser.textureLoader).toBeInstanceOf(THREE.TextureLoader)
  })

  it('mantém o LoadingManager do parser, que é quem contabiliza o progresso da carga', () => {
    const manager = new THREE.LoadingManager()
    const parser: TextureLoaderHost = { textureLoader: null, options: { manager } }

    forceImgTextureLoader(parser)

    expect((parser.textureLoader as THREE.TextureLoader).manager).toBe(manager)
  })
})
