import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { openComposer } from '../fixtures/composer'
import { expect, test } from '../fixtures/tauri-app'
import { waitForUiIdle } from '../fixtures/settled'

const SCREENSHOTS_DIR = join(process.cwd(), 'e2e/screenshots')
function shot(name: string) {
  return join(SCREENSHOTS_DIR, `${name}.png`)
}

type BridgeWindow = Window & {
  electronAPI?: {
    runtime: {
      getTools: () => Promise<{ tools: Array<{ name: string; description?: string }> }>
      executeTool: (
        name: string,
        args: Record<string, unknown>,
      ) => Promise<{
        success: boolean
        data?: {
          monitorCount?: number
          images?: unknown[]
          analysis?: string
          foregroundApp?: string
          response?: string
        }
        error?: unknown
      }>
      saveSetting: (k: string, v: unknown) => Promise<unknown>
      onAssistantMessage: (cb: (msg: { content: string }) => void) => void
      onChatError: (cb: (err: { message: string }) => void) => void
      transcribeAudio: (opts: { audioBase64: string; mimeType: string }) => Promise<unknown>
      speakText: (text: string) => Promise<{ audio?: unknown; mimeType?: string }>
    }
  }
}

async function waitForBridge(page: Page) {
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as BridgeWindow).electronAPI))).toBe(true)
}

test.describe('Homologação — Chat + Tools + Vision + Voice', () => {
  test.beforeAll(() => {
    mkdirSync(SCREENSHOTS_DIR, { recursive: true })
  })

  test('1. Chat com Ollama + tools via qwen2.5:7b', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const { tools } = await p.evaluate(
      () => (window as unknown as BridgeWindow).electronAPI?.runtime.getTools() ?? { tools: [] },
    )
    expect(tools.length).toBeGreaterThan(0)
    writeFileSync(
      join(SCREENSHOTS_DIR, 'tools-catalog.json'),
      JSON.stringify(
        tools.map((t) => ({ name: t.name, desc: t.description?.slice(0, 80) })),
        null,
        2,
      ),
    )

    const snapshot = await p.evaluate(async () =>
      (window as unknown as BridgeWindow).electronAPI?.runtime.executeTool('memory_console', { action: 'snapshot' }),
    )
    expect(snapshot?.success).toBe(true)
    writeFileSync(join(SCREENSHOTS_DIR, 'tool-snapshot.json'), JSON.stringify({ data: snapshot?.data }, null, 2))

    const pending = p.evaluate(async () => {
      const api = (window as unknown as BridgeWindow).electronAPI?.runtime
      return new Promise<{ content: string }>((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Timeout 120s')), 120_000)
        api?.onAssistantMessage((msg) => {
          clearTimeout(timeout)
          resolve({ content: msg.content })
        })
        api?.onChatError((err) => {
          clearTimeout(timeout)
          reject(new Error(err.message))
        })
      })
    })

    const composer = await openComposer(p)
    await composer.fill('Ola Mika! Quem e voce? Responde em ate 20 palavras.')
    await composer.press('Enter')

    const result = await pending
    expect(result.content).toBeTruthy()
    expect(result.content.length).toBeGreaterThan(10)
    writeFileSync(join(SCREENSHOTS_DIR, 'chat-response.json'), JSON.stringify(result, null, 2))

    await waitForUiIdle(p)
    await p.screenshot({ path: shot('chat-01-response'), fullPage: true })
  })

  test('2. CaptureScreen + ferramentas de sistema', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await p.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.saveSetting('vision.enabled', true))
    await expect
      .poll(() =>
        p.evaluate(
          async () =>
            (
              await (
                window as unknown as {
                  electronAPI: {
                    runtime: { getSettingsConfig: () => Promise<{ config: { vision?: { enabled?: boolean } } }> }
                  }
                }
              ).electronAPI.runtime.getSettingsConfig()
            ).config.vision?.enabled,
        ),
      )
      .toBe(true)

    const capture = await p.evaluate(async () => {
      try {
        const res = await (window as unknown as BridgeWindow).electronAPI?.runtime.executeTool('vision_capture', {
          action: 'frame',
          quality: 50,
        })
        return {
          success: true,
          monitorCount: res?.data?.monitorCount ?? 0,
          hasImages: Boolean(res?.data?.images?.length),
        }
      } catch (e) {
        return { success: false, error: String(e).slice(0, 200) }
      }
    })

    writeFileSync(join(SCREENSHOTS_DIR, 'tool-capturescreen.json'), JSON.stringify(capture, null, 2))

    const analysis = await p.evaluate(async () => {
      try {
        const res = await (window as unknown as BridgeWindow).electronAPI?.runtime.executeTool('vision_read', {
          action: 'describe',
          imageBase64:
            'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
          prompt: 'Descreva esta imagem.',
        })
        return { success: true, analysis: res?.data?.analysis?.slice(0, 300) }
      } catch (e) {
        return { success: false, error: String(e).slice(0, 200) }
      }
    })
    writeFileSync(join(SCREENSHOTS_DIR, 'vision-analysis.json'), JSON.stringify(analysis, null, 2))
  })

  test('3. vision_capture/watch_now respeita o consentimento e responde pelo modelo de visão', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await p.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.saveSetting('vision.enabled', true))

    const denied = await p.evaluate(async () =>
      (window as unknown as BridgeWindow).electronAPI?.runtime.executeTool('vision_capture', {
        action: 'watch_now',
        question: 'O que estou vendo?',
      }),
    )
    expect(denied?.success).toBe(false)
    // Rust adapter deliberately collapses privacy denial, blocklist, and real capture failure into
    // the SAME response (src-tauri/src/modules/vision/responses.rs, `capture_unavailable_result`) —
    // the caller never learns which one it was, mirroring the TS `captureScreen` contract. The old
    // assertion expected a privacy-specific PT-BR message that this collapsed shape never returns.
    expect(String(denied?.error)).toContain('Screen capture returned empty result')

    await p.evaluate(() =>
      (window as unknown as BridgeWindow).electronAPI?.runtime.saveSetting('privacy.screenCapture', true),
    )
    const granted = await p.evaluate(async () =>
      (window as unknown as BridgeWindow).electronAPI?.runtime.executeTool('vision_capture', {
        action: 'watch_now',
        question: 'O que estou vendo?',
      }),
    )
    expect(granted?.error ?? null).toBeNull()
    expect(granted?.success).toBe(true)
    expect(String(granted?.data?.response ?? '').length).toBeGreaterThan(0)

    writeFileSync(
      join(SCREENSHOTS_DIR, 'screen-observation.json'),
      JSON.stringify(
        {
          deniedError: denied?.error,
          foregroundApp: granted?.data?.foregroundApp,
          response: String(granted?.data?.response ?? '').slice(0, 500),
        },
        null,
        2,
      ),
    )
  })

  test('4. Voice STT + TTS via IPC', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const sttResult = await p.evaluate(async () => {
      try {
        return await (window as unknown as BridgeWindow).electronAPI?.runtime.transcribeAudio({
          audioBase64: '',
          mimeType: 'audio/webm',
        })
      } catch (e) {
        return { success: false, error: String(e).slice(0, 200) }
      }
    })
    writeFileSync(join(SCREENSHOTS_DIR, 'voice-stt.json'), JSON.stringify(sttResult, null, 2))

    const ttsResult = await p.evaluate(async () => {
      try {
        const r = await (window as unknown as BridgeWindow).electronAPI?.runtime.speakText('Ola, eu sou a Mika!')
        return { hasAudio: Boolean(r?.audio), mimeType: r?.mimeType }
      } catch (e) {
        return { success: false, error: String(e).slice(0, 200) }
      }
    })
    writeFileSync(join(SCREENSHOTS_DIR, 'voice-tts.json'), JSON.stringify(ttsResult, null, 2))
  })
})
