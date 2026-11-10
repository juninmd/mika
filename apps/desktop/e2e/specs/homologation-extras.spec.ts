import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { openComposer } from '../fixtures/composer'
import { expect, test, type TauriShellApp, waitForStableExecutionContext } from '../fixtures/tauri-app'
import { enableShellChrome } from '../fixtures/shell-chrome'
import { clickDockButton } from '../fixtures/dock'
import { waitForUiIdle } from '../fixtures/settled'

const SCREENSHOTS_DIR = join(process.cwd(), 'e2e/screenshots')
function shotPath(name: string) {
  return join(SCREENSHOTS_DIR, `${name}.png`)
}

type BridgeWindow = Window & {
  electronAPI?: {
    runtime: {
      saveSetting: (k: string, v: unknown) => Promise<unknown>
      getSettingsConfig: () => Promise<{ config: Record<string, Record<string, unknown> | undefined> }>
      getPluginUi: () => Promise<{ views: Array<{ pluginId: string; id: string }> }>
      getUiPrefs?: () => Promise<{ locale?: string; theme?: string }>
    }
    shell?: {
      pluginViews: {
        open: (opts: { pluginId: string; viewId: string }) => Promise<unknown>
      }
    }
  }
}

async function waitForBridge(page: Page) {
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as BridgeWindow).electronAPI))).toBe(true)
}

async function openWindow(app: TauriShellApp, openFn: () => Promise<void>): Promise<Page> {
  const winPromise = app.waitForEvent('window')
  await openFn()
  const win = await winPromise
  await win.waitForLoadState('domcontentloaded')
  return win
}

const BUILTIN_TAB_NAV: Array<{ id: string; section: string; btnLabel: string }> = [
  { id: 'overview', section: 'Sistema', btnLabel: 'Resumo' },
  { id: 'modules', section: 'Sistema', btnLabel: 'Módulos' },
  { id: 'homologation', section: 'Sistema', btnLabel: 'Diagnóstico' },
  { id: 'logs', section: 'Sistema', btnLabel: 'Logs' },
  { id: 'updates', section: 'Sistema', btnLabel: 'Atualizações' },
  { id: 'audio-devices', section: 'Voz', btnLabel: 'Dispositivos de Áudio' },

  { id: 'skin', section: 'Mika', btnLabel: 'Skin' },
  { id: 'credentials', section: 'Conta', btnLabel: 'Credenciais' },
  { id: 'tools-mcp', section: 'Ferramentas', btnLabel: 'Catálogo' },
]

// The settings window is a fresh CDP target — same "context destroyed mid-navigation" race the
// fixture already absorbs for the main window (`waitForStableExecutionContext` in
// `../fixtures/tauri-app.ts`), just not covered there for secondary windows. Retry locally instead
// of widening the shared fixture.
async function railSectionLabels(settings: Page): Promise<string[]> {
  for (let attempt = 0; attempt < 5; attempt++) {
    try {
      return await settings
        .getByRole('navigation')
        .getByRole('button')
        .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('aria-label') ?? ''))
    } catch (error) {
      if (!String(error).includes('Execution context was destroyed') || attempt === 4) throw error
      await waitForStableExecutionContext(settings, 10_000)
    }
  }
  return []
}

async function navigateSettingsBuiltin(settings: Page, tab: (typeof BUILTIN_TAB_NAV)[0]) {
  await settings.getByRole('navigation').getByRole('button', { name: tab.section, exact: true }).click()
  const tabButton = settings.locator('button').filter({ hasText: tab.btnLabel }).first()
  await expect(tabButton).toBeVisible()
  await tabButton.click()
  await waitForUiIdle(settings)
}

test.describe('Homologação Extras — Missing Interfaces', () => {
  test.beforeAll(() => {
    mkdirSync(SCREENSHOTS_DIR, { recursive: true })
  })

  test('1. Built-in Settings Panels', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const settings = await openWindow(app, async () => {
      await clickDockButton(p, 'Configurações')
    })

    await expect.poll(async () => (await railSectionLabels(settings)).length).toBeGreaterThan(5)
    const railSections = await railSectionLabels(settings)
    expect(BUILTIN_TAB_NAV.filter((tab) => !railSections.includes(tab.section))).toEqual([])

    for (const tab of BUILTIN_TAB_NAV) {
      await navigateSettingsBuiltin(settings, tab)
      await waitForUiIdle(settings)
      await settings.screenshot({ path: shotPath(`settings-builtin-${tab.id}`), fullPage: true, timeout: 10000 })
      await expect(settings.getByText('Manual panel required')).toHaveCount(0)
    }

    await settings.close()
  })

  test('3. Wizard — caminho alternativo (pular IA, voltar)', async ({ app }) => {
    test.setTimeout(60_000)
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    // Perfil já configurado (LLM fake): o wizard é o "refazer o setup", que PRESERVA o que o usuário já
    // tinha (`hadExistingSetup`). Guarda mic/memória de antes para provar que o passo não os apagou.
    const readSenses = () =>
      p.evaluate(async () => {
        const res = await (window as unknown as BridgeWindow).electronAPI?.runtime.getSettingsConfig()
        const config = res?.config ?? {}
        return { sttEnabled: config.voice?.sttEnabled, memoryEnabled: config.memory?.enabled }
      })
    const sensesBefore = await readSenses()

    const winPromise = app.waitForEvent('window')
    await p.evaluate(() =>
      (window as unknown as BridgeWindow).electronAPI?.runtime.saveSetting('setup.wizardCompleted', false),
    )
    const wizard = await winPromise
    await wizard.waitForLoadState('domcontentloaded')
    await expect(wizard.getByText('Onboarding da Mika')).toBeVisible({ timeout: 10_000 })

    const next = (name = 'Continuar') => wizard.getByRole('button', { name, exact: true }).click()

    // 1 idioma → 2 avatar → 3 cérebro (pula a IA) → 4 voz → 5 escuta → 6 memória → 7 privacidade → 8 identidade.
    await expect(wizard.getByText('Etapa 1 de 8')).toBeVisible()
    await next()
    await expect(wizard.getByText('Etapa 2 de 8')).toBeVisible()
    await next()
    await expect(wizard.getByText('Etapa 3 de 8')).toBeVisible()
    await next('Explorar sem configurar IA')
    await expect(wizard.getByText('Etapa 4 de 8')).toBeVisible()
    await wizard.screenshot({ path: shotPath('wizard-alt-02-tts'), fullPage: true })

    // Voltar do passo de voz devolve ao cérebro, e pular de novo segue para a voz.
    await next('Voltar')
    await expect(wizard.getByText('Etapa 3 de 8')).toBeVisible()
    await next('Explorar sem configurar IA')
    await expect(wizard.getByText('Etapa 4 de 8')).toBeVisible()

    await next()
    await expect(wizard.getByText('Etapa 5 de 8')).toBeVisible()
    await next()
    await expect(wizard.getByText('Etapa 6 de 8')).toBeVisible()
    await next()
    await expect(wizard.getByText('Etapa 7 de 8')).toBeVisible()
    await wizard.screenshot({ path: shotPath('wizard-alt-01-privacy'), fullPage: true })
    await next('Salvar escolhas e continuar')
    await expect(wizard.getByText('Etapa 8 de 8')).toBeVisible()

    const privacy = await p.evaluate(async () => {
      const res = await (window as unknown as BridgeWindow).electronAPI?.runtime.getSettingsConfig()
      const config = res?.config ?? {}
      return {
        screenCapture: config.privacy?.screenCapture,
        clipboardLogging: config.privacy?.clipboardLogging,
        visionEnabled: config.vision?.enabled,
        readClipboardImages: config.vision?.readClipboardImages,
        visualDiaryEnabled: config.vision?.visualDiaryEnabled,
        sttEnabled: config.voice?.sttEnabled,
        memoryEnabled: config.memory?.enabled,
      }
    })
    // Tela, clipboard e visão seguem desligados; mic e memória ficam como o usuário já os tinha.
    expect(privacy).toEqual({
      screenCapture: false,
      clipboardLogging: false,
      visionEnabled: false,
      readClipboardImages: false,
      visualDiaryEnabled: false,
      sttEnabled: sensesBefore.sttEnabled,
      memoryEnabled: sensesBefore.memoryEnabled,
    })

    await wizard.getByRole('button', { name: 'Concluir setup', exact: true }).click()

    await expect(wizard.getByText('Mika pronta')).toBeVisible()
    await expect(wizard.getByText('Configuração pendente')).toBeVisible()
    await wizard.screenshot({ path: shotPath('wizard-alt-03-done'), fullPage: true })

    // The last step had no way back: a failed download left the user stuck on it.
    const back = wizard.getByRole('button', { name: 'Voltar', exact: true })
    await expect(back).toBeEnabled({ timeout: 120_000 })
    await back.click()
    await expect(wizard.getByText('Etapa 8 de 8')).toBeVisible()
    await wizard.getByRole('button', { name: 'Concluir setup', exact: true }).click()
    await expect(wizard.getByText('Mika pronta')).toBeVisible()

    await wizard.getByRole('button', { name: 'Entrar no desktop', exact: true }).click()
  })

  test('4. Plugin View Declarations + UI', async ({ app }) => {
    test.setTimeout(180_000)
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const catalog = await p.evaluate(() => (window as unknown as BridgeWindow).electronAPI?.runtime.getPluginUi())
    expect(catalog, 'a ponte não devolveu o catálogo de views de plugin').toBeTruthy()
    expect(catalog?.views.length).toBeGreaterThanOrEqual(1)

    const opened: string[] = []
    for (const view of catalog?.views ?? []) {
      const viewWin = await openWindow(app, async () => {
        await p.evaluate(
          ({ pid, vid }: { pid: string; vid: string }) =>
            (window as unknown as BridgeWindow).electronAPI?.shell?.pluginViews.open({ pluginId: pid, viewId: vid }),
          { pid: view.pluginId, vid: view.id },
        )
      })
      await viewWin.waitForLoadState('domcontentloaded')
      await waitForUiIdle(viewWin)

      // Janela de plugin tem de enxergar idioma e tema. Quando `getSettingsConfig` era o caminho, a
      // guarda de superfície negava, o idioma caía no padrão e o tema escolhido não pintava aqui.
      const prefs = await viewWin.evaluate(() =>
        (window as unknown as BridgeWindow).electronAPI?.runtime.getUiPrefs?.(),
      )
      expect(prefs, `janela de plugin ${view.id} sem acesso a idioma/tema`).toBeTruthy()

      const paint = await viewWin.evaluate(() => ({
        lang: document.documentElement.lang,
        bg: getComputedStyle(document.documentElement).getPropertyValue('--bg').trim(),
        text: getComputedStyle(document.documentElement).getPropertyValue('--text').trim(),
      }))
      expect(paint.lang, `janela de plugin ${view.id} sem idioma aplicado`).not.toBe('')
      expect(paint.bg, `janela de plugin ${view.id} sem tema pintado`).not.toBe('')
      expect(paint.text).not.toBe('')

      const safeName = `${view.pluginId}-${view.id}`.replace(/[^a-z0-9]/gi, '-')
      await viewWin.screenshot({ path: shotPath(`plugin-view-${safeName}`), fullPage: true })
      await viewWin.close()
      opened.push(view.id)
    }

    expect(opened).toEqual(catalog?.views.map((v: { id: string }) => v.id))
  })

  test('5. Desktop Shell Overlays', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await openComposer(p)
    await p.screenshot({ path: shotPath('overlay-chat-input'), fullPage: true })
    await p.keyboard.press('Escape')

    await enableShellChrome(p)

    await p.keyboard.press('Control+k')
    await expect(p.getByPlaceholder('Buscar comandos e ferramentas…')).toBeVisible()
    await p.screenshot({ path: shotPath('overlay-command-palette'), fullPage: true })

    await expect(p.getByText('get system info')).toBeVisible()
    await p.screenshot({ path: shotPath('overlay-tools-panel'), fullPage: true })
    await p.keyboard.press('Escape')
  })

  test('6. Desktop Shell Dock Close-up', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await p.screenshot({ path: shotPath('overlay-dock-buttons'), fullPage: true })
  })
})
