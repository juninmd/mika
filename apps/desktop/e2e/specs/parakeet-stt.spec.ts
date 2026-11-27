import {
  copyFileSync,
  existsSync,
  linkSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
} from 'node:fs'
import { dirname, join } from 'node:path'
import type { Page } from '@playwright/test'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { snapTop, waitForUiIdle } from '../fixtures/settled'
import { expect, test as base } from '../fixtures/tauri-app'

// Relative to the spec, like every other artifact here.
const SCREENSHOTS_DIR = join(__dirname, '..', 'screenshots')
const FRAMES_DIR = join(SCREENSHOTS_DIR, 'parakeet-stt-frames')
const SPEECH_WAV = join(__dirname, '..', 'fixtures', 'voice', 'fala-usuario-pt-br.wav')
const MAX_FRAMES = 60
/** B should contribute enough frames for a readable GIF on its own (A already writes 3). */
const MIN_B_FRAMES = 15
// Capture cadence for the GIF the reviewer assembles afterwards. This is a capture interval, not a
// "let the UI settle" sleep — assertions still use expect.poll / toBeVisible.
const FRAME_INTERVAL_MS = 300

const PARAKEET_FILES = ['encoder-model.int8.onnx', 'decoder_joint-model.int8.onnx', 'nemo128.onnx', 'vocab.txt'] as const
const NEMOTRON_FILES = [
  'encoder.onnx',
  'encoder.onnx.data',
  'decoder.onnx',
  'decoder.onnx.data',
  'joint.onnx',
  'joint.onnx.data',
  'vocab.txt',
] as const
const WHISPER_FILES = ['encoder_model_quantized.onnx', 'decoder_model_merged_quantized.onnx', 'tokenizer.json'] as const

const PARAKEET_DIR = process.env.MIKA_E2E_PARAKEET_DIR
const NEMOTRON_DIR = process.env.MIKA_E2E_NEMOTRON_DIR
const WHISPER_DIR = process.env.MIKA_E2E_WHISPER_DIR

type ShellWindow = { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }

function hasFiles(dir: string | undefined, files: readonly string[]): dir is string {
  return Boolean(dir) && files.every((file) => existsSync(join(dir as string, file)))
}

const PARAKEET_READY = hasFiles(PARAKEET_DIR, PARAKEET_FILES)
const NEMOTRON_READY = hasFiles(NEMOTRON_DIR, NEMOTRON_FILES)
const WHISPER_READY = hasFiles(WHISPER_DIR, WHISPER_FILES)

function missingReason(variable: string, dir: string | undefined, files: readonly string[]): string {
  if (!dir) return `${variable} não definido`
  return `${variable} não aponta para uma pasta com ${files.join(', ')}`
}

/**
 * The caches are read-only to the app and some are ~670 MB: hardlink first (same volume, no extra
 * bytes), then a file symlink, and only fall back to a copy when the OS refuses both. The temp
 * profile cleanup removes the link, never the source.
 */
function linkOrCopy(source: string, target: string): void {
  rmSync(target, { force: true })
  mkdirSync(dirname(target), { recursive: true })
  try {
    linkSync(source, target)
    return
  } catch {
    // EXDEV across volumes, EPERM on Windows without developer mode.
  }
  try {
    symlinkSync(source, target, 'file')
    return
  } catch {
    // Same cases; copy is the portable fallback.
  }
  copyFileSync(source, target)
}

function materializeDir(source: string, target: string): void {
  mkdirSync(target, { recursive: true })
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    const from = join(source, entry.name)
    const to = join(target, entry.name)
    if (entry.isDirectory()) materializeDir(from, to)
    else if (entry.isFile()) linkOrCopy(from, to)
  }
}

const VAD_MODEL = join(__dirname, '..', 'fixtures', 'models', 'silero_vad.onnx')

const test = base.extend({
  profile: async ({ profile }, use) => {
    // The Rust `voice:vad-audio` session opens Silero before any STT path, whatever `vadEngine`
    // says; without this file every audio chunk logs "modelo do VAD ausente" and barge-in dies.
    linkOrCopy(VAD_MODEL, join(profile.userDataDir, 'vad-cache', 'silero_vad.onnx'))
    if (PARAKEET_READY && PARAKEET_DIR) materializeDir(PARAKEET_DIR, join(profile.userDataDir, 'parakeet-cache'))
    if (NEMOTRON_READY && NEMOTRON_DIR) materializeDir(NEMOTRON_DIR, join(profile.userDataDir, 'nemotron-cache'))
    if (WHISPER_READY && WHISPER_DIR) materializeDir(WHISPER_DIR, join(profile.userDataDir, 'whisper-cache', 'whisper-base'))
    // Chromium/WebView2 reads this when it launches the fake microphone, i.e. after the profile
    // fixture and before `app` — the same WAV live-transcript uses.
    process.env.MIKA_E2E_FAKE_AUDIO = SPEECH_WAV
    await use(profile)
  },
})

// The fake microphone is set in the profile fixture (before `app` spawns). Clear it after each
// test so a later spec in the same worker never inherits the env by accident.
test.afterEach(() => {
  delete process.env.MIKA_E2E_FAKE_AUDIO
})

// The frames tell one story (settings → listening → transcript); clear leftovers from a previous
// run so the numbering does not mix two narratives.
test.beforeAll(() => {
  rmSync(FRAMES_DIR, { recursive: true, force: true })
})

async function openSettings(page: Page): Promise<Page> {
  const settingsPromise = page.context().waitForEvent('page')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')
  return settings
}

async function navigateToTab(settings: Page, query: string, heading: string | RegExp): Promise<void> {
  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill(query)
  await settings.keyboard.press('Enter')
  await expect(settings.getByRole('heading', { name: heading }).first()).toBeVisible()
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)
}

async function readConfigPath(page: Page, path: string): Promise<unknown> {
  return page.evaluate(async (p) => {
    const { config } = await (window as unknown as ShellWindow).electronAPI.runtime.getSettingsConfig()
    return p.split('.').reduce<unknown>((node, key) => (node as Record<string, unknown> | undefined)?.[key], config)
  }, path)
}

function nextFrameIndex(): number {
  if (!existsSync(FRAMES_DIR)) return 1
  const indices = readdirSync(FRAMES_DIR)
    .map((file) => /^(\d{3})\.png$/.exec(file)?.[1])
    .filter((value): value is string => value !== undefined)
    .map(Number)
  return indices.length === 0 ? 1 : Math.max(...indices) + 1
}

type FrameSequence = { dir: string; next: number }

function frameSequence(): FrameSequence {
  mkdirSync(FRAMES_DIR, { recursive: true })
  return { dir: FRAMES_DIR, next: nextFrameIndex() }
}

async function captureFrame(page: Page, sequence: FrameSequence): Promise<void> {
  const path = join(sequence.dir, `${String(sequence.next).padStart(3, '0')}.png`)
  sequence.next += 1
  await page.screenshot({ path })
}

/** Cadence only: the interval is what makes the GIF show "listening → transcript". */
async function captureUntil(
  page: Page,
  sequence: FrameSequence,
  done: () => boolean,
  maxFrames: number,
  minFrames: number,
): Promise<void> {
  for (let captured = 0; captured < maxFrames; captured += 1) {
    await captureFrame(page, sequence)
    if (captured + 1 >= minFrames && done()) return
    await new Promise((resolve) => setTimeout(resolve, FRAME_INTERVAL_MS))
  }
}

function readWavPcm16(path: string): Buffer {
  const wav = readFileSync(path)
  const dataAt = wav.indexOf(Buffer.from('data'))
  if (dataAt < 0) throw new Error(`${path}: chunk data ausente`)
  const size = wav.readUInt32LE(dataAt + 4)
  const pcm = wav.subarray(dataAt + 8, dataAt + 8 + size)
  if (pcm.length === 0 || pcm.length % 2 !== 0) throw new Error(`${path}: PCM16 vazio ou ímpar (${pcm.length} bytes)`)
  return pcm
}

async function transcribePcm(
  page: Page,
  pcmBase64: string,
): Promise<{ success: boolean; text: string; error?: string; ms: number }> {
  return page.evaluate(async (audioBase64) => {
    const bytes = Uint8Array.from(atob(audioBase64), (char) => char.charCodeAt(0))
    const runtime = (window as unknown as ShellWindow).electronAPI.runtime
    const started = performance.now()
    const result = await runtime.transcribeAudio({ audio: bytes, mimeType: 'audio/pcm;rate=16000', language: 'pt' })
    return {
      success: result.success,
      text: result.data?.text ?? '',
      error: result.error,
      ms: performance.now() - started,
    }
  }, pcmBase64)
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b)
  return Math.round(sorted[Math.floor(sorted.length / 2)] ?? 0)
}

test.describe('Parakeet STT', () => {
  test('A — escolher Parakeet pela UI, confirmar a config, o card pronto e a atribuição', async ({ app }) => {
    test.skip(!PARAKEET_READY, missingReason('MIKA_E2E_PARAKEET_DIR', PARAKEET_DIR, PARAKEET_FILES))
    test.setTimeout(240_000)
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')

    const settings = await openSettings(page)
    await navigateToTab(settings, 'Escuta', /Escuta|STT/)

    const frames = frameSequence()
    await waitForUiIdle(settings)
    await captureFrame(settings, frames)

    // The real provider card, clicked like the user does — no saveSetting shortcut.
    const parakeet = settings.getByRole('button', { name: /Parakeet v3/ })
    await expect(parakeet).toBeVisible()
    await parakeet.click()
    await expect(parakeet).toHaveAttribute('aria-pressed', 'true')
    await expect.poll(() => readConfigPath(settings, 'voice.asrProvider')).toBe('parakeet')
    await waitForUiIdle(settings)
    await captureFrame(settings, frames)

    // Details panel: the download/verify button reaches the real `prepareWizardAsset` for the
    // hardlinked cache, so "pronto" proves the files on disk, not just the button label.
    const download = settings.getByRole('button', { name: /Baixar\/verificar modelo/ })
    await expect(download).toBeVisible()
    await download.click()
    await expect(settings.getByText('Modelo Parakeet pronto')).toBeVisible({ timeout: 120_000 })
    await expect(settings.getByText('Parakeet TDT 0.6B v3 © NVIDIA, CC-BY-4.0')).toBeVisible()

    const details = settings.getByText('Parakeet TDT 0.6B v3 © NVIDIA, CC-BY-4.0').locator('xpath=ancestor::section[1]')
    await details.scrollIntoViewIfNeeded()
    await waitForUiIdle(settings)
    await captureFrame(settings, frames)

    await snapTop(settings, artifactPath('parakeet-stt-settings.png'))
    await details.scrollIntoViewIfNeeded()
    await waitForUiIdle(settings)
    await details.screenshot({ path: artifactPath('parakeet-stt-settings-card.png') })
  })

  test('B — com Parakeet escolhido, a fala do fixture vira um turno de voz no Histórico', async ({ app }) => {
    test.skip(!PARAKEET_READY, missingReason('MIKA_E2E_PARAKEET_DIR', PARAKEET_DIR, PARAKEET_FILES))
    test.setTimeout(240_000)
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')

    // Turn listening OFF first: the profile boots with the mic on and the default engine, so any
    // turn still in flight would otherwise be a Nemotron transcript without Parakeet punctuation.
    await page.evaluate(async () => {
      await (window as unknown as ShellWindow).electronAPI.runtime.saveSetting('voice.sttEnabled', false)
    })
    await expect(page.getByRole('button', { name: 'Ativar Modo de Voz (F9)' })).toBeVisible({ timeout: 20_000 })

    await page.locator('#root > div').first().dispatchEvent('contextmenu', { clientX: 200, clientY: 200 })
    await page.getByRole('menu').getByRole('menuitem', { name: /^Histórico/i }).click()
    const history = page.locator('[role="dialog"][aria-labelledby="chat-history-title"]')
    await expect(history).toBeVisible()

    const spoken = history.locator('article').filter({ has: page.getByRole('img', { name: /voz alta/i }) })
    const before = await spoken.count()

    // Provider and endpointing are saved while listening is OFF, so the first new turn after the
    // switch is decoded by Parakeet. `sttEnabled` comes back last.
    await page.evaluate(async () => {
      const runtime = (window as unknown as ShellWindow).electronAPI.runtime
      await runtime.saveSetting('voice.asrProvider', 'parakeet')
      await runtime.saveSetting('voice.vadSilenceMs', 600)
      await runtime.saveSetting('voice.inputMode', 'continuous')
      await runtime.saveSetting('voice.vadEndpointingEnabled', true)
    })
    await expect.poll(() => readConfigPath(page, 'voice.asrProvider')).toBe('parakeet')

    const frames = frameSequence()
    let done = false
    const maxFrames = Math.max(0, MAX_FRAMES - (frames.next - 1))
    const minFrames = Math.min(MIN_B_FRAMES, maxFrames)
    // Start the GIF cadence before the mic comes back so the frames show Ouvindo você… appearing.
    const capturing = captureUntil(page, frames, () => done, maxFrames, minFrames).catch(() => undefined)

    await page.evaluate(async () => {
      await (window as unknown as ShellWindow).electronAPI.runtime.saveSetting('voice.sttEnabled', true)
    })
    await expect.poll(() => readConfigPath(page, 'voice.sttEnabled')).toBe(true)

    try {
      await expect.poll(() => spoken.count(), { timeout: 180_000 }).toBeGreaterThan(before)

      // The FIRST turn after the count. Do not use `.last()`: the looping fake audio can append a
      // later turn between reading the text and taking the screenshot.
      const finalTurn = spoken.nth(before)
      await expect(finalTurn).toBeVisible()
      const speech = finalTurn.locator('.whitespace-pre-wrap').first()
      await expect
        .poll(async () => ((await speech.textContent()) ?? '').trim().length, { timeout: 60_000 })
        .toBeGreaterThan(0)
      // Only the message paragraph: no "Você" label and no timestamp from the article container.
      const text = ((await speech.textContent()) ?? '').trim()
      expect(text.toLowerCase()).toMatch(/voltou|diferente/)
      // Punctuation is Parakeet's signature against the unpunctuated Nemotron transcript.
      expect(text).toMatch(/[,\.?]/)

      done = true
      await capturing

      await finalTurn.scrollIntoViewIfNeeded()
      await waitForUiIdle(page)
      await captureFrame(page, frames)
      await page.screenshot({ path: artifactPath('parakeet-stt-transcript.png') })
      writeArtifact('parakeet-stt', { provider: 'parakeet', text })
    } finally {
      done = true
      await capturing
    }
  })

  test('C — latência pelo IPC real para cada motor local com pesos', async ({ app }) => {
    test.skip(
      !PARAKEET_READY && !NEMOTRON_READY && !WHISPER_READY,
      'nenhum MIKA_E2E_*_DIR com pesos (Parakeet, Nemotron ou Whisper)',
    )
    test.setTimeout(420_000)
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')

    // The fake microphone is a file (see the hook above); turn the continuous loop off so only the
    // explicit IPC calls below consume the CPU while the samples are measured. The VAD gate is also
    // disabled: this test measures STT latency, and a stale no-voice verdict from the fake mic would
    // reject a PCM call before it ever reaches the engine.
    await page.evaluate(async () => {
      const runtime = (window as unknown as ShellWindow).electronAPI.runtime
      await runtime.saveSetting('voice.sttEnabled', false)
      await runtime.saveSetting('voice.vadEngine', 'energy')
      await runtime.saveSetting('voice.vadIgnoreTyping', false)
    })
    await expect(page.getByRole('button', { name: 'Ativar Modo de Voz (F9)' })).toBeVisible({ timeout: 20_000 })

    const pcmBase64 = readWavPcm16(SPEECH_WAV).toString('base64')
    const entries: Array<
      | { provider: string; texto: string; ms: number[]; mediana: number }
      | { provider: string; omitido: string }
    > = []

    const providers = [
      { id: 'parakeet', ready: PARAKEET_READY, reason: missingReason('MIKA_E2E_PARAKEET_DIR', PARAKEET_DIR, PARAKEET_FILES) },
      { id: 'nemotron', ready: NEMOTRON_READY, reason: missingReason('MIKA_E2E_NEMOTRON_DIR', NEMOTRON_DIR, NEMOTRON_FILES) },
      { id: 'local-whisper', ready: WHISPER_READY, reason: missingReason('MIKA_E2E_WHISPER_DIR', WHISPER_DIR, WHISPER_FILES) },
    ]

    for (const provider of providers) {
      if (!provider.ready) {
        entries.push({ provider: provider.id, omitido: provider.reason })
        continue
      }
      await page.evaluate(async (value) => {
        await (window as unknown as ShellWindow).electronAPI.runtime.saveSetting('voice.asrProvider', value)
      }, provider.id)
      await expect.poll(() => readConfigPath(page, 'voice.asrProvider')).toBe(provider.id)

      const warmup = await transcribePcm(page, pcmBase64)
      if (!warmup.success) throw new Error(`${provider.id}: aquecimento falhou: ${warmup.error ?? 'sem detalhe'}`)

      const ms: number[] = []
      let texto = ''
      for (let run = 0; run < 3; run += 1) {
        const result = await transcribePcm(page, pcmBase64)
        if (!result.success) throw new Error(`${provider.id}: medição ${run + 1} falhou: ${result.error ?? 'sem detalhe'}`)
        ms.push(result.ms)
        texto = result.text
      }
      entries.push({ provider: provider.id, texto, ms, mediana: median(ms) })
    }

    const parakeet = entries.find(
      (entry): entry is { provider: string; texto: string; ms: number[]; mediana: number } =>
        entry.provider === 'parakeet' && 'texto' in entry,
    )
    if (PARAKEET_READY) {
      expect(parakeet, 'Parakeet disponível mas ausente do JSON').toBeDefined()
      expect(parakeet?.texto.toLowerCase()).toContain('ela voltou')
    }

    writeArtifact('parakeet-stt-latency', entries)
  })
})
