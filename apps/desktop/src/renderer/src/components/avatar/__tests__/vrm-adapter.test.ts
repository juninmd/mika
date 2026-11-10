import { beforeEach, describe, expect, it, vi } from 'vitest'

// playAction/setExpression are the only paths under test; the renderer/GL stack is mocked away.
const rendererLogWarn = vi.fn()
const rendererLogDebug = vi.fn()
vi.mock('../../../lib/renderer-logger', () => ({
  rendererLog: { warn: rendererLogWarn, debug: rendererLogDebug, info: vi.fn(), error: vi.fn() },
}))

vi.mock('three', async (importOriginal) => {
  const actual = await importOriginal<typeof import('three')>()
  return {
    ...actual,
    WebGLRenderer: vi.fn().mockImplementation(function FakeWebGLRenderer(this: Record<string, unknown>) {
      this.setClearColor = vi.fn()
      this.setPixelRatio = vi.fn()
      this.setSize = vi.fn()
      this.render = vi.fn()
      this.dispose = vi.fn()
    }),
  }
})

vi.mock('../vrm-postfx', () => ({
  AnimePostFx: vi.fn().mockImplementation(function FakeAnimePostFx(this: Record<string, unknown>) {
    this.syncSize = vi.fn()
    this.setQuality = vi.fn()
    this.trackGlow = vi.fn()
    this.render = vi.fn()
    this.dispose = vi.fn()
  }),
}))

const deepDispose = vi.fn()
vi.mock('@pixiv/three-vrm', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@pixiv/three-vrm')>()
  return { ...actual, VRMUtils: { ...actual.VRMUtils, deepDispose } }
})

const loadVrmGltf = vi.hoisted(() => vi.fn())
vi.mock('../vrm-scene-setup', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../vrm-scene-setup')>()),
  loadVrmGltf,
}))

const { VrmAvatar } = await import('../vrm-adapter')

function makeAvatar() {
  // vitest runs this suite under the node environment (no DOM); devicePixelRatio is the only global read.
  vi.stubGlobal('window', { devicePixelRatio: 1 })
  return new VrmAvatar({} as HTMLCanvasElement, 400, 640)
}

describe('VrmAvatar', () => {
  beforeEach(() => {
    rendererLogWarn.mockClear()
    rendererLogDebug.mockClear()
  })

  it('playAction with an unknown name logs a warning', () => {
    const avatar = makeAvatar()
    avatar.playAction('not-a-real-action')
    expect(rendererLogWarn).toHaveBeenCalledWith(
      '[VrmAvatar] ação recusada, nome desconhecido',
      expect.objectContaining({ name: 'not-a-real-action' }),
    )
  })

  it('playAction with a known name does not warn', () => {
    const avatar = makeAvatar()
    avatar.playAction('nod')
    expect(rendererLogWarn).not.toHaveBeenCalled()
  })

  it('setExpression drops the pose with a debug log when another action is still playing', () => {
    const avatar = makeAvatar()
    avatar.playAction('sitting')
    rendererLogDebug.mockClear()
    avatar.setExpression('happy', 1)
    expect(rendererLogDebug).toHaveBeenCalledWith(
      '[VrmAvatar] pose descartada, ação em andamento',
      expect.objectContaining({ remaining: expect.any(Number) }),
    )
  })

  // Regression: a model that finished downloading after the avatar was disposed (stage unmount, skin switch)
  // was attached to a dead scene and its geometry/textures were never freed.
  it('frees a model that resolves after dispose() instead of attaching it', async () => {
    const avatar = makeAvatar()
    let resolveLoad: (value: unknown) => void = () => {}
    loadVrmGltf.mockReturnValue(new Promise((resolve) => (resolveLoad = resolve)))
    const vrmScene = { name: 'late-vrm-scene' }
    const attach = vi.spyOn(avatar.scene, 'add')

    const loading = avatar.load('late.vrm')
    avatar.dispose()
    resolveLoad({ gltf: { scene: vrmScene }, vrm: { scene: vrmScene } })
    await loading

    expect(deepDispose).toHaveBeenCalledExactlyOnceWith(vrmScene)
    expect(attach).not.toHaveBeenCalled()
    expect(avatar.modelFrame()).toBeNull()
  })
})
