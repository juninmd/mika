import { t } from '@mika/sdk/i18n'
import { toErrorMessage } from '@mika/sdk/utils/object'
import type * as PIXI from 'pixi.js'

const CUBISM_CORE_PATH = 'live2dcubismcore.min.js'

type Live2DModelInstance = PIXI.Container & {
  expression: (name?: string | number) => void
  motion: (group: string, index: number, priority?: number) => Promise<boolean>
  update: (delta: number) => void
  internalModel: {
    settings?: { name?: string; url?: string }
    motionManager?: { stopAllMotions: () => void }
    /** Cubism physics (absent when the skin ships no physics3.json); its options carry the wind force. */
    physics?: { getOption?: () => { wind: { x: number; y: number } } }

    width?: number
    height?: number
    coreModel: {
      setParameterValueById: (id: string, value: number) => void
      getParameterValueById?: (id: string) => number | null
    }
  }
}

function resolveRendererAssetUrl(assetPath: string): string {
  return new URL(assetPath.replace(/^\//, ''), document.baseURI).toString()
}

/**
 * Modelos próprios (`live2d/<nome>/...`, `.model3.json` ou `.vrm`) moram em `<userData>/live2d/`,
 * onde o "Importar modelo" (`import_model.rs`) os copia, e saem pelo esquema custom `mika-live2d`
 * (`src-tauri/src/live2d_protocol.rs`) também em dev: nenhum modelo de terceiro vem no bundle.
 *
 * Mapeamento de plataforma do próprio Tauri para um esquema custom: Windows/Android usam
 * `http://<esquema>.localhost/<path>`; macOS/Linux usam `<esquema>://localhost/<path>`.
 */
export function resolveLive2DAssetUrl(assetPath: string, platform: string = process.platform): string {
  const normalized = assetPath.replace(/^\.\//, '').replace(/^\//, '')
  // The scheme root already is <userData>/live2d; the bundle-relative prefix would double it.
  const relative = normalized.replace(/^live2d\//, '')
  const origin = platform === 'win32' ? 'http://mika-live2d.localhost' : 'mika-live2d://localhost'
  return `${origin}/${relative}`
}

function waitForScriptLoad(script: HTMLScriptElement): Promise<void> {
  return new Promise((resolve, reject) => {
    script.addEventListener('load', () => resolve(), { once: true })
    script.addEventListener('error', () => reject(new Error(`Failed to load ${script.src}`)), { once: true })
  })
}

async function ensureCubismCoreLoaded(): Promise<void> {
  if ((window as unknown as Record<string, unknown>).Live2DCubismCore) return

  const existing = document.querySelector<HTMLScriptElement>('script[data-live2d-core="true"]')
  if (existing) {
    await waitForScriptLoad(existing)
  } else {
    const script = document.createElement('script')
    script.src = resolveRendererAssetUrl(CUBISM_CORE_PATH)
    script.dataset.live2dCore = 'true'
    document.head.appendChild(script)
    await waitForScriptLoad(script)
  }

  if (!(window as unknown as Record<string, unknown>).Live2DCubismCore) {
    throw new Error(t('shell.avatar.cubismCoreMissing'))
  }
}

function normalizeModelPath(modelPath = ''): string {
  return modelPath.replace(/^\.\//, '').replace(/^\//, '')
}

const STAGE_HEIGHT_FILL = 0.95
const STAGE_WIDTH_FILL = 1.0

export function fitModelToStage(model: Live2DModelInstance, stageWidth: number, stageHeight: number): void {
  const container = model as unknown as PIXI.Container
  container.scale.set(1)

  const naturalWidth = model.internalModel?.width || container.width
  const naturalHeight = model.internalModel?.height || container.height
  if (!(naturalWidth > 0) || !(naturalHeight > 0)) {
    container.scale.set(0.18)
    return
  }

  const scale = Math.min(
    (stageHeight * STAGE_HEIGHT_FILL) / naturalHeight,
    (stageWidth * STAGE_WIDTH_FILL) / naturalWidth,
  )
  container.scale.set(scale)
}

export function layoutModelForStage(model: Live2DModelInstance, stageWidth: number, stageHeight: number): void {
  fitModelToStage(model, stageWidth, stageHeight)
  ;(model as unknown as { anchor?: { set: (x: number, y: number) => void } }).anchor?.set(0.5, 1.0)
  const container = model as unknown as PIXI.Container
  container.x = stageWidth * 0.5
  container.y = stageHeight - stageHeight * 0.005
}

export function resolveModelStem(model: Live2DModelInstance): string | undefined {
  const settings = model.internalModel?.settings

  const url = settings?.url
  if (url) {
    const file = url.split(/[\\/]/).pop() ?? ''
    const stem = file.replace('.model3.json', '')
    if (stem) return stem
  }

  return settings?.name || undefined
}

/** Texture-owning destroy: Pixi leaves GPU textures behind unless `texture`/`baseTexture` are asked for. */
export const PIXI_FULL_DESTROY = { children: true, texture: true, baseTexture: true } as const

/** The stage unmounted while the model was loading: nothing owns it, so free it instead of parenting it. */
export function discardIfCancelled(model: Live2DModelInstance, isCancelled: () => boolean): boolean {
  if (!isCancelled()) return false
  model.destroy(PIXI_FULL_DESTROY)
  return true
}

export async function loadLive2DModel(
  app: PIXI.Application,
  modelPath?: string,
  isCancelled: () => boolean = () => false,
): Promise<Live2DModelInstance | null> {
  await ensureCubismCoreLoaded()
  const { Live2DModel } = await import('pixi-live2d-display/cubism4')

  const modelUrl = resolveLive2DAssetUrl(normalizeModelPath(modelPath))
  const screenWidth = app.screen?.width || 320
  const screenHeight = app.screen?.height || 350

  // No Live2D ships with the app: a broken user model surfaces as an error, not a silent swap.
  let model: Live2DModelInstance
  try {
    model = (await Live2DModel.from(modelUrl)) as unknown as Live2DModelInstance
  } catch (error: unknown) {
    throw new Error(`Falha ao carregar Live2D em ${modelUrl}: ${toErrorMessage(error)}`)
  }

  if (discardIfCancelled(model, isCancelled)) return null

  layoutModelForStage(model, screenWidth, screenHeight)

  app.stage.addChild(model as PIXI.Container)
  return model
}

export type { Live2DModelInstance }
