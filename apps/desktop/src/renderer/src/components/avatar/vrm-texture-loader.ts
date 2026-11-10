import * as THREE from 'three'

// On `file://`, Chromium rejects the blob fetch the default ImageBitmapLoader uses, so embedded VRM textures failed silently.
export interface TextureLoaderHost {
  textureLoader: unknown
  options?: { manager?: THREE.LoadingManager }
}

export function forceImgTextureLoader(parser: TextureLoaderHost): void {
  parser.textureLoader = new THREE.TextureLoader(parser.options?.manager)
}
