import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import type { Page } from '@playwright/test'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { openComposer } from '../fixtures/composer'
import { callE2eHook, expect, test, type TauriShellApp } from '../fixtures/tauri-app'
import { clickDockButton } from '../fixtures/dock'
import { snapTop, waitForUiIdle } from '../fixtures/settled'

// Single continuous session proving the full journey closes in sequence, not just per-surface (see homologation specs).
// clear_history runs via runtime.executeTool because the fake LLM only proposes one tool call per conversation.

const SCREENSHOTS_DIR = join(process.cwd(), 'e2e/screenshots')
function shot(name: string): string {
  return join(SCREENSHOTS_DIR, `journey-${name}.png`)
}

type BridgeWindow = Window & { electronAPI?: ElectronShellApi }

async function waitForBridge(page: Page): Promise<void> {
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as BridgeWindow).electronAPI))).toBe(true)
}

async function openWindow(app: TauriShellApp, openFn: () => Promise<void>): Promise<Page> {
  const winPromise = app.waitForEvent('window')
  await openFn()
  const win = await winPromise
  await win.waitForLoadState('domcontentloaded')
  return win
}

async function openSettings(app: TauriShellApp, page: Page): Promise<Page> {
  return openWindow(app, async () => {
    await clickDockButton(page, 'Configurações')
  })
}

async function navigateSettingsTab(settings: Page, query: string): Promise<void> {
  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill(query)
  await settings.keyboard.press('Enter')
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)
}

async function flipSwitch(settings: Page, label: string | RegExp): Promise<void> {
  const sw = settings.getByRole('switch', typeof label === 'string' ? { name: label, exact: true } : { name: label })
  await expect(sw).toBeVisible()
  const before = await sw.getAttribute('aria-checked')
  await sw.click()
  const after = before === 'true' ? 'false' : 'true'
  await expect(sw).toHaveAttribute('aria-checked', after, { timeout: 5000 })
}

async function getConfig(page: Page): Promise<Record<string, any>> {
  const res = await page.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.getSettingsConfig())
  return (res?.config ?? {}) as Record<string, any>
}

async function executeTool<T = unknown>(
  page: Page,
  name: string,
  args: Record<string, unknown>,
): Promise<{ success: boolean; data?: T; error?: string } | undefined> {
  return page.evaluate(({ n, a }) => (window as unknown as BridgeWindow).electronAPI?.runtime.executeTool(n, a), {
    n: name,
    a: args,
  }) as Promise<{ success: boolean; data?: T; error?: string } | undefined>
}

// Must be called before sending the turn so the listener is registered before the event fires.
function waitForAssistantReply(page: Page): Promise<{ content: string }> {
  return page.evaluate(() => {
    const api = (window as unknown as BridgeWindow).electronAPI?.runtime
    return new Promise<{ content: string }>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Timeout aguardando resposta do assistente (60s)')), 60_000)
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
}

function waitForToolLoopReply(
  page: Page,
): Promise<{ content: string; calls: string[]; results: Array<{ name: string; success?: boolean }> }> {
  return page.evaluate(() => {
    const api = (window as unknown as BridgeWindow).electronAPI?.runtime
    const calls: string[] = []
    const results: Array<{ name: string; success?: boolean }> = []
    api?.onToolCall((p) => calls.push(p.name))
    api?.onToolResult((p) => results.push({ name: p.name, success: p.success }))
    return new Promise<{ content: string; calls: string[]; results: Array<{ name: string; success?: boolean }> }>(
      (resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Timeout aguardando laço de tools (60s)')), 60_000)
        api?.onAssistantMessage((msg) => {
          clearTimeout(timeout)
          resolve({ content: msg.content, calls, results })
        })
        api?.onChatError((err) => {
          clearTimeout(timeout)
          reject(new Error(err.message))
        })
      },
    )
  })
}

test.describe('Jornada do usuário — todas as features numa sessão só', () => {
  test.beforeAll(() => {
    mkdirSync(SCREENSHOTS_DIR, { recursive: true })
  })

  test('um boot, uma conversa, as 9 features do roster + núcleo, desligamento gracioso', async ({ app }) => {
    test.setTimeout(360_000)

    const page = await app.firstWindow()

    await test.step('1. Boot + onboarding — dismissar o modal de privacidade', async () => {
      await page.waitForLoadState('domcontentloaded')
      await waitForBridge(page)

      const health = await page.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.getHealth())
      expect(health).toHaveProperty('status')
      expect(health?.features).toBeGreaterThan(0)

      // Sem esperar o avatar o print sai em branco: o boot termina antes do VRM carregar.
      await page
        .waitForFunction(() => Boolean((window as unknown as { __mikaVrm?: unknown }).__mikaVrm), undefined, {
          timeout: 30_000,
        })
        .catch(() => undefined)
      await snapTop(page, shot('01-boot'))
    })

    await test.step('2. Chat — mensagem simples, sem tools', async () => {
      const pending = waitForAssistantReply(page)
      const composer = await openComposer(page)
      await composer.fill('Oi Mika! Bora começar essa sessão — em poucas palavras, o que você faz?')
      await composer.press('Enter')

      const result = await pending
      expect(result.content.length).toBeGreaterThan(0)
      await waitForUiIdle(page)
      await snapTop(page, shot('02-chat'))
    })

    await test.step('3. Trigger de tool call via chat (get_system_info)', async () => {
      const pending = waitForToolLoopReply(page)
      const composer = await openComposer(page)
      await composer.fill('HOMOLOGA_TOOL_LOOP: como está a máquina, Mika?')
      await composer.press('Enter')

      const result = await pending
      expect(result.calls).toContain('get_system_info')
      expect(result.results.find((r) => r.name === 'get_system_info')?.success).toBe(true)
      expect(result.content.length).toBeGreaterThan(0)
      await waitForUiIdle(page)
      await snapTop(page, shot('03-tool-loop-via-chat'))
    })

    await test.step('4. Tool de alto risco — NEGAR (via runtime.executeTool, mesmo portão do chat)', async () => {
      const pending = executeTool(page, 'clear_history', { confirm: true })

      const dialog = page.getByRole('dialog')
      await expect(dialog).toBeVisible({ timeout: 20_000 })
      await expect(page.getByText('clear_history')).toBeVisible()
      await snapTop(page, shot('04-tool-approval-deny'))

      await page.getByRole('button', { name: /Negar/ }).click()

      const result = await pending
      expect(result?.success).toBe(false)
      await expect(dialog).toHaveCount(0)
    })

    await test.step('5. Configurações — um toggle em cada uma das 9 features do roster', async () => {
      const settings = await openSettings(app, page)

      await navigateSettingsTab(settings, 'Personagem')
      await expect(settings.getByRole('heading', { name: /Personagem/i }).first()).toBeVisible()
      await expect(settings.getByText(/Comportamento|Sarcasmo|Caos|Tsundere/i).first()).toBeVisible()
      await flipSwitch(settings, 'Tom informal')
      await snapTop(settings, shot('05-settings-character'))

      await navigateSettingsTab(settings, 'Gamer Companion')
      await expect(settings.getByRole('heading', { name: /Gamer Companion/i }).first()).toBeVisible()
      await flipSwitch(settings, 'Detecção automática')
      await snapTop(settings, shot('05-settings-gamer-companion'))

      await navigateSettingsTab(settings, 'MCP')
      await expect(settings.getByRole('heading', { name: /MCP/i }).first()).toBeVisible()
      await flipSwitch(settings, 'MCP')
      await snapTop(settings, shot('05-settings-mcp'))

      // Round trip: ends active because step 8 depends on it.
      await navigateSettingsTab(settings, 'mood')
      await expect(settings.getByText(/Sistema de Humor|Humor e Afinidade/i).first()).toBeVisible()
      await flipSwitch(settings, 'Sistema de Humor ativo')
      await expect(settings.getByText(/Desligado|Inativo/).first()).toBeVisible()
      await flipSwitch(settings, 'Sistema de Humor ativo')
      await expect(settings.getByText('Ativo').first()).toBeVisible()
      await snapTop(settings, shot('05-settings-mood'))

      // Round trip: ends active because step 6 depends on it.
      await navigateSettingsTab(settings, 'person-catalog')
      await expect(settings.getByText(/Catálogo de Pessoas/i).first()).toBeVisible()
      // O interruptor do cabeçalho do módulo leva o nome da aba (`aria-label` = tabLabel).
      await flipSwitch(settings, 'Catálogo de Pessoas')
      await expect(settings.getByText(/Desligado|Inativo/).first()).toBeVisible()
      await flipSwitch(settings, 'Catálogo de Pessoas')
      await expect(settings.getByText('Ativo').first()).toBeVisible()
      await snapTop(settings, shot('05-settings-person-catalog'))

      await navigateSettingsTab(settings, 'Privacidade')
      await expect(settings.getByRole('heading', { name: /Privacidade/i }).first()).toBeVisible()
      await flipSwitch(settings, /Log da [áa]rea de transfer[êe]ncia/i)
      await snapTop(settings, shot('05-settings-privacy'))

      // Ends off: enabled would unregister ReminderStore and make clear_history (step 11) fail partially.
      await navigateSettingsTab(settings, 'Lembretes')
      await expect(settings.getByRole('heading', { name: /Lembretes/i }).first()).toBeVisible()
      await flipSwitch(settings, 'Lembretes')
      await flipSwitch(settings, 'Lembretes')
      await snapTop(settings, shot('05-settings-scheduler'))

      // No boolean in this schema; edit a text field and confirm persistence instead.
      await navigateSettingsTab(settings, 'Usuário')
      await expect(settings.getByRole('heading', { name: /Usuário/i }).first()).toBeVisible()
      const nickname = settings.getByPlaceholder('Como gosta de ser chamado')
      await expect(nickname).toBeVisible()
      await nickname.fill('Jornada E2E')
      await nickname.press('Tab')
      await expect.poll(async () => (await getConfig(page)).user?.nickname).toBe('Jornada E2E')
      await snapTop(settings, shot('05-settings-user'))

      await settings.close()

      // desktop is tools-only, no settings tab; action=window is non-destructive, runs without a dialog.
      const desktopResult = await executeTool(page, 'desktop', { action: 'window' })
      expect(desktopResult?.success).toBe(true)
      await snapTop(page, shot('05-settings-desktop-tool'))
    })

    await test.step('6. Memória e Catálogo de Pessoas', async () => {
      const snapshot = await executeTool(page, 'memory_console', { action: 'snapshot' })
      expect(snapshot?.success).toBe(true)

      const upsert = await executeTool<{ personId: string }>(page, 'person_catalog', {
        action: 'upsert',
        name: 'Jornada E2E Persona',
        type: 'fictional_character',
      })
      expect(upsert?.success).toBe(true)

      const listed = await executeTool<Array<{ primaryName: string }>>(page, 'person_catalog', {
        action: 'list',
        limit: 50,
      })
      expect(listed?.success).toBe(true)
      expect((listed?.data ?? []).map((p) => p.primaryName)).toContain('Jornada E2E Persona')

      await snapTop(page, shot('06-memory-person-catalog'))
    })

    await test.step('7. Live2D / skin — ação do bridge de debug + expressão via tool do personagem', async () => {
      // Without GPU the avatar falls back with no debug bridge installed; only this section is skipped.
      const canvas = page.locator('.live2d-container canvas, canvas.live2d-container')
      const mounted = await canvas
        .waitFor({ state: 'attached', timeout: 15_000 })
        .then(() => true)
        .catch(() => false)

      const engine = await page.evaluate(() => {
        const w = window as unknown as { __mikaLive2D?: unknown; __mikaVrm?: unknown }
        if (w.__mikaLive2D) return 'live2d'
        if (w.__mikaVrm) return 'vrm'
        return 'none'
      })

      if (mounted && engine === 'vrm') {
        // Expression changing alone doesn't prove the action ran; must read the arm angle.
        type VrmBridge = {
          playAction: (n: string) => void
          readArmRotations: () => Record<string, number>
        }
        const readArm = () =>
          page.evaluate(
            () => (window as unknown as { __mikaVrm?: VrmBridge }).__mikaVrm?.readArmRotations().rightUpperArm ?? 0,
          )
        const rest = await readArm()
        await page.evaluate(() => (window as unknown as { __mikaVrm?: VrmBridge }).__mikaVrm?.playAction('wave'))

        await expect
          .poll(async () => Math.abs((await readArm()) - rest), {
            message: 'a ação "wave" não tirou o braço direito do repouso',
            timeout: 5000,
          })
          .toBeGreaterThan(0.1)
        await snapTop(page, shot('07-vrm-wave'))
      } else if (mounted) {
        const actions = await page.evaluate(
          () =>
            (window as unknown as { __mikaLive2D?: { listActions: () => string[] } }).__mikaLive2D?.listActions() ?? [],
        )
        expect(actions.length).toBeGreaterThan(0)

        const played = await page.evaluate(
          (name) =>
            (window as unknown as { __mikaLive2D?: { playAction: (n: string) => boolean } }).__mikaLive2D?.playAction(
              name,
            ) ?? false,
          'wave',
        )
        expect(played).toBe(true)
        await waitForUiIdle(page)
        await snapTop(page, shot('07-live2d-wave'))
      } else {
        test.info().annotations.push({
          type: 'skip-reason',
          description: 'Sem WebGL neste ambiente — avatar em fallback, sem canvas para o bridge de debug.',
        })
        await snapTop(page, shot('07-live2d-unavailable-no-webgl'))
      }

      // Independent of the canvas: emitted by the EventBus, doesn't depend on render.
      const expressionPending = page.evaluate(() => {
        const api = (window as unknown as BridgeWindow).electronAPI?.runtime
        return new Promise<{ expression: string }>((resolve, reject) => {
          const timeout = setTimeout(() => reject(new Error('Timeout aguardando avatar:expression (10s)')), 10_000)
          // Só a expressão pedida: o reset para `neutral` do passo anterior também passa por este canal.
          api?.onAvatarExpression((p) => {
            if (p.expression !== 'happy') return
            clearTimeout(timeout)
            resolve(p)
          })
        })
      })
      const expressionResult = await executeTool(page, 'avatar', {
        action: 'expression',
        expression: 'happy',
        durationMs: 2000,
      })
      expect(expressionResult?.success).toBe(true)
      const expressionEvent = await expressionPending
      expect(expressionEvent.expression).toBe('happy')
      await waitForUiIdle(page)
      await snapTop(page, shot('07-avatar-expression-happy'))
    })

    await test.step('8. Mudança de humor', async () => {
      const moodPending = page.evaluate(() => {
        const api = (window as unknown as BridgeWindow).electronAPI?.runtime
        return new Promise<{ valence: number; arousal: number; label: string }>((resolve, reject) => {
          const timeout = setTimeout(() => reject(new Error('Timeout aguardando mood:changed (10s)')), 10_000)
          api?.onMoodChanged((p) => {
            clearTimeout(timeout)
            resolve(p)
          })
        })
      })
      const moodResult = await executeTool(page, 'mood', { action: 'update', valence: 0.7, arousal: 0.5 })
      expect(moodResult?.success).toBe(true)

      const moodEvent = await moodPending
      expect(moodEvent.valence).toBeCloseTo(0.7, 1)
      expect(moodEvent.arousal).toBeCloseTo(0.5, 1)
      await snapTop(page, shot('08-mood-change'))
    })

    await test.step('9. Vision / screen-watcher — consentimento em Privacidade cascateia para Visão', async () => {
      const settings = await openSettings(app, page)

      // The Privacy panel saves vision.enabled together when screenCapture is turned on.
      await navigateSettingsTab(settings, 'Privacidade')
      await flipSwitch(settings, 'Captura de Tela')
      await snapTop(settings, shot('09-privacy-screen-capture'))

      await navigateSettingsTab(settings, 'Vision')
      await expect(settings.getByRole('heading', { name: /Visão/i }).first()).toBeVisible()
      const visionSwitch = settings.getByRole('switch', { name: 'Visão', exact: true })
      await expect(visionSwitch).toHaveAttribute('aria-checked', 'true')
      // Round trip, ends enabled for the tool call below.
      await flipSwitch(settings, 'Visão')
      await flipSwitch(settings, 'Visão')
      await expect(visionSwitch).toHaveAttribute('aria-checked', 'true')
      await snapTop(settings, shot('09-vision-settings'))
      await settings.close()

      // No E2E a janela em foco é a própria Mika, que a captura recusa de propósito: o jogador de verdade
      // está em outro app. Põe um jogo no snapshot de janela ativa; o gate (consentimento, blocklist,
      // janela própria) e a captura de monitor seguem reais.
      await callE2eHook(page, 'set-active-window', { title: 'Hollow Knight', owner: 'hollow_knight' })
      const watch = await executeTool<{ response?: string; foregroundApp?: unknown }>(page, 'vision_capture', {
        action: 'watch_now',
        question: 'O que você vê na tela agora?',
      })
      expect(watch?.error ?? null).toBeNull()
      expect(watch?.success).toBe(true)
      expect(String(watch?.data?.response ?? '').length).toBeGreaterThan(0)
      await snapTop(page, shot('09-vision-watch-now'))
    })

    await test.step('10. Configurações de voz — Fala automática', async () => {
      const settings = await openSettings(app, page)
      await navigateSettingsTab(settings, 'Fala')
      await expect(settings.getByRole('heading', { name: /Fala|TTS/ }).first()).toBeVisible()
      // O harness sobe com a voz desligada (sem áudio no E2E); "Fala automática" só habilita com "Mika fala".
      await flipSwitch(settings, 'Mika fala')
      await flipSwitch(settings, 'Fala automática')
      await snapTop(settings, shot('10-voice-settings'))
      await settings.close()
    })

    await test.step('11. Encerramento gracioso — aprova o clear_history e confirma que o app segue de pé', async () => {
      const pending = executeTool(page, 'clear_history', { confirm: true })

      const dialog = page.getByRole('dialog')
      await expect(dialog).toBeVisible({ timeout: 20_000 })
      await expect(page.getByText('clear_history')).toBeVisible()
      await snapTop(page, shot('11-tool-approval-approve'))

      await page.getByRole('button', { name: 'Permitir', exact: true }).click()

      const result = await pending
      expect(result?.success).toBe(true)
      await expect(dialog).toHaveCount(0)

      // Final health check proves the app still responds after the session's most destructive action.
      const health = await page.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.getHealth())
      expect(health?.status).toBeTruthy()
      await snapTop(page, shot('12-shutdown-health'))

      // A prova de que o clear_history agiu: a memória episódica que a Privacidade lista ficou vazia.
      const settings = await openSettings(app, page)
      await navigateSettingsTab(settings, 'Privacidade')
      await expect(settings.getByText('Nenhuma memória registrada ainda.')).toBeVisible()
      await snapTop(settings, shot('12-memory-cleared'))
      await settings.close()
    })
  })
})
