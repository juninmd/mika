import { spawnSync } from 'node:child_process'
import { cpSync, existsSync, mkdirSync, statSync, writeFileSync } from 'node:fs'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { APP_BINARY, launchCaptureApp } from '../lib/tauri-capture-app.mts'
import {
  assertPreviewsAreDistinct,
  assertSkinIdentity,
  CONTAINER,
  findAvatarWindow,
  stretchStageToWindow,
  waitForAvatar,
} from './thumbs-avatar.mts'
import { assertThumbWidth, CAPTURE_WINDOW, faceClip, measurePaintedBox, shrinkToThumb } from './thumbs-image.mts'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
const DESKTOP = join(ROOT, 'apps', 'desktop')
const LIVE2D_PUBLIC = join(DESKTOP, 'src', 'renderer', 'public', 'live2d')
const RENDERER_LIVE2D = join(DESKTOP, 'src', 'renderer', 'src', 'components', 'live2d')

type ModelOption = {
  id: string
  label: string
  path: string
  catalogKey: string
  bundled?: boolean
  preview?: string
}

const { LIVE2D_MODELS } = (await import(pathToFileURL(join(RENDERER_LIVE2D, 'model-path.ts')).href)) as {
  LIVE2D_MODELS: readonly ModelOption[]
}
const { LIVE2D_ACTION_CATALOG } = (await import(
  pathToFileURL(join(RENDERER_LIVE2D, 'live2d-action-catalog.generated.ts')).href
)) as { LIVE2D_ACTION_CATALOG: Record<string, Record<string, unknown>> }

const args = process.argv.slice(2)

function flag(name: string): string | undefined {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : undefined
}

const onlyModel = flag('--model')
const freezeSeconds = Number(flag('--seconds') ?? 1.5)

function seedProfile(userDataDir: string, model: ModelOption): Record<string, unknown> {
  // Third-party Live2D samples are not bundled; the app serves them from `<userData>/live2d/<name>/`.
  if (!model.bundled) {
    const folder = model.path.split('/')[1] as string
    mkdirSync(join(userDataDir, 'live2d', folder), { recursive: true })
    cpSync(join(LIVE2D_PUBLIC, folder), join(userDataDir, 'live2d', folder), { recursive: true })
  }
  return {
    setup: { wizardCompleted: true },
    telemetry: { optInShown: true },
    character: { enabled: true, modelPath: model.path, backgroundMode: 'transparent' },
    llm: { provider: 'ollama', model: 'mika-thumbs' },
    apiKeys: { ollamaBaseUrl: 'http://127.0.0.1:9' },
    voice: { ttsEnabled: false, sttEnabled: false },
    vision: { enabled: false },
    brain: { enabled: false },
  }
}

function previewFileFor(model: ModelOption): string {
  return join(LIVE2D_PUBLIC, model.path.split('/')[1] as string, 'preview.png')
}

function captureInChildProcess(model: ModelOption): string {
  const child = spawnSync(
    process.execPath,
    [...process.execArgv, fileURLToPath(import.meta.url), '--model', model.id],
    { stdio: 'inherit' },
  )
  if (child.status !== 0) {
    throw new Error(`o subprocesso saiu com ${child.status ?? child.signal ?? 'erro desconhecido'}`)
  }
  return previewFileFor(model)
}

async function captureModel(model: ModelOption): Promise<string> {
  const userDataDir = await mkdtemp(join(tmpdir(), 'mika-thumbs-'))
  const app = await launchCaptureApp(userDataDir, seedProfile(userDataDir, model))
  const fallbacks: string[] = []

  try {
    const page = await findAvatarWindow(app.pages, fallbacks)
    // tsx wraps named inner functions in `__name`, a helper that does not exist in the page.
    await page.evaluate('globalThis.__name = (fn) => fn')
    await app.setSize(CAPTURE_WINDOW.width, CAPTURE_WINDOW.height)

    await waitForAvatar(page)
    await assertSkinIdentity(page, model, fallbacks, LIVE2D_ACTION_CATALOG)
    await stretchStageToWindow(page)

    await page.evaluate((seconds) => {
      const bridge = (
        window as unknown as { __mikaLive2D?: { freeze(name: string | null, seconds: number): Promise<boolean> } }
      ).__mikaLive2D

      return bridge?.freeze(null, seconds) ?? false
    }, freezeSeconds)

    await page.addStyleTag({
      content: `html, body { background: transparent !important }
                body *:not(${CONTAINER}):not(${CONTAINER} *) { visibility: hidden !important }
                ${CONTAINER}, ${CONTAINER} * { visibility: visible !important }`,
    })

    const box = (await page.locator(CONTAINER).boundingBox()) ?? { x: 0, y: 0, ...(page.viewportSize() ?? {}) }
    if (!('width' in box) || !box.width || !box.height) {
      throw new Error('o palco do avatar mediu 0x0 — a janela ainda não tinha layout')
    }

    const viewport = page.viewportSize() ?? { width: box.width, height: box.height }
    const frame = await page.screenshot({ clip: box, scale: 'css', omitBackground: true })
    const painted = await measurePaintedBox(page, frame, box)
    if (!painted) {
      throw new Error('o canvas do avatar não tem um pixel opaco — a skin não chegou a ser desenhada')
    }

    const clip = faceClip(painted, viewport)
    const target = previewFileFor(model)

    const shot = await page.screenshot({ clip, scale: 'css', omitBackground: true })
    const thumb = await shrinkToThumb(page, shot)
    assertThumbWidth(model, thumb)
    writeFileSync(target, thumb)
    return target
  } finally {
    await app.close()
    await rm(userDataDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 }).catch(() => undefined)
  }
}

if (!existsSync(APP_BINARY)) {
  console.error(`[live2d:thumbs] ${APP_BINARY} não existe. Rode antes: cd apps/desktop && npx tauri build --no-bundle`)
  process.exit(1)
}

const targets = onlyModel ? LIVE2D_MODELS.filter((model) => model.id === onlyModel) : LIVE2D_MODELS
if (targets.length === 0) {
  console.error(
    `[live2d:thumbs] nenhum modelo com id '${onlyModel}'. Ids: ${LIVE2D_MODELS.map((m) => m.id).join(', ')}`,
  )
  process.exit(1)
}

const failures: string[] = []
const generated: { label: string; file: string }[] = []
for (const model of targets) {
  try {
    const file = onlyModel ? await captureModel(model) : captureInChildProcess(model)
    generated.push({ label: model.label, file })
    if (onlyModel)
      console.log(`[live2d:thumbs] ${model.label} → ${file} (${Math.round(statSync(file).size / 1024)} KB)`)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    failures.push(`${model.label}: ${message}`)
    console.error(`[live2d:thumbs] ${model.label} FALHOU: ${message}`)
  }
}

if (failures.length > 0) {
  console.error(`[live2d:thumbs] ${failures.length} de ${targets.length} miniaturas não foram geradas.`)
  process.exit(1)
}

assertPreviewsAreDistinct(generated)

const totalKb = Math.round(generated.reduce((sum, { file }) => sum + statSync(file).size, 0) / 1024)
console.log(`[live2d:thumbs] ${targets.length} miniaturas geradas em ${LIVE2D_PUBLIC} (${totalKb} KB no total)`)
