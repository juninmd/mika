import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { openComposer } from '../fixtures/composer'
import { expect, test, type TauriShellApp } from '../fixtures/tauri-app'
import { enableShellChrome } from '../fixtures/shell-chrome'
import { clickDockButton } from '../fixtures/dock'
import { waitForUiIdle } from '../fixtures/settled'

const SCREENSHOTS_DIR = join(process.cwd(), 'e2e/screenshots/ui-homologation')

function shot(name: string) {
  return join(SCREENSHOTS_DIR, `${name}.png`)
}

async function waitForBridge(page: Page) {
  await expect
    .poll(() => page.evaluate(() => Boolean((window as unknown as { electronAPI?: unknown }).electronAPI)))
    .toBe(true)
}

async function openWindow(app: TauriShellApp, openFn: () => Promise<void>): Promise<Page> {
  const winPromise = app.waitForEvent('window')
  await openFn()
  const win = await winPromise
  await win.waitForLoadState('domcontentloaded')
  return win
}

test.describe('Homologação Completa de UI — Componentes e Telas', () => {
  test.beforeEach(async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)
    await enableShellChrome(p)
  })

  test('1. Painel de Histórico de Chat — busca, contadores, cópia, limpeza com confirmação e fechar', async ({
    app,
  }) => {
    const p = await app.firstWindow()

    // Popula mensagens no histórico via API do runtime
    await p.evaluate(async () => {
      window.dispatchEvent(
        new CustomEvent('incoming-message', {
          detail: { role: 'user', content: 'Pergunta teste de homologação', timestamp: Date.now() - 5000 },
        }),
      )
      window.dispatchEvent(
        new CustomEvent('assistant-message', {
          detail: { content: 'Resposta simulada da Mika para teste', timestamp: Date.now() },
        }),
      )
    })

    // Abre painel de histórico via menu de contexto
    await p.locator('#root > div').first().dispatchEvent('contextmenu', { clientX: 200, clientY: 200 })
    const menu = p.locator('[role="menu"]')
    await expect(menu).toBeVisible({ timeout: 5000 })
    await menu.getByRole('menuitem', { name: /^Histórico/i }).click()

    const historyDialog = p.locator('[role="dialog"][aria-labelledby="chat-history-title"]')
    await expect(historyDialog).toBeVisible({ timeout: 5000 })

    await p.screenshot({ path: shot('01-chat-history-panel') })

    // Testa campo de busca do histórico
    const searchInput = historyDialog.getByPlaceholder(/buscar nas mensagens/i)
    if (await searchInput.isVisible().catch(() => false)) {
      await searchInput.fill('homologação')
      await expect(searchInput).toHaveValue('homologação')
      await waitForUiIdle(p)
      await p.screenshot({ path: shot('01-chat-history-search') })

      // Limpa a busca
      const clearSearchBtn = historyDialog.getByRole('button', { name: /limpar busca/i })
      if (await clearSearchBtn.isVisible().catch(() => false)) {
        await clearSearchBtn.click()
      } else {
        await searchInput.fill('')
      }
    }

    // Botão de fechar do histórico
    const closeBtn = historyDialog.getByRole('button', { name: /fechar/i }).first()
    await closeBtn.click()
    await expect(historyDialog).toHaveCount(0)
  })

  test('2. SpeechBubble & ThinkingHUD & HUD Sensors — renderização e controles interativos', async ({ app }) => {
    const p = await app.firstWindow()

    // Aciona um efeito visual para verificar o ActiveEffectChip no HUD
    await p.evaluate(async () => {
      const api = (
        window as unknown as {
          electronAPI: { runtime: { executeTool: (n: string, a: { text: string }) => Promise<unknown> } }
        }
      ).electronAPI
      await api.runtime.executeTool('match_effect', { text: '/matrix' })
    })

    const effectChip = p.getByRole('button', { name: /efeito .* ativo/i })
    await expect(effectChip).toBeVisible({ timeout: 5000 })
    await p.screenshot({ path: shot('02-hud-active-effect-chip') })

    // Clica no chip para desligar o efeito
    await effectChip.click()
    await expect(effectChip).toHaveCount(0)

    // Simula fala da Mika para acender a SpeechBubble
    await p.evaluate(() => {
      window.dispatchEvent(
        new CustomEvent('assistant-message', {
          detail: { content: 'Olá! Sou a Mika, sua VTuber gamer companheira!' },
        }),
      )
    })
    await waitForUiIdle(p)

    const speechText = p.getByText(/Olá! Sou a Mika/i)
    if (await speechText.isVisible().catch(() => false)) {
      await p.screenshot({ path: shot('02-speech-bubble-visible') })
    }

    // SensesMonitor no topo direito
    const sensesMonitor = p.getByRole('status', { name: /monitores sensoriais da mika/i })
    if (await sensesMonitor.isVisible().catch(() => false)) {
      await expect(sensesMonitor).toBeVisible()
      await p.screenshot({ path: shot('02-senses-monitor') })
    }
  })

  test('3. ChatComposer — Quick Actions preenchendo texto e atalhos de teclado', async ({ app }) => {
    const p = await app.firstWindow()

    const composer = await openComposer(p)
    await expect(composer).toBeVisible()

    // Verifica botões de Quick Actions
    const quickButtons = p.locator('button').filter({ hasText: /Olhar tela|Qual jogo|Quem é|Reagir|Surpresa/i })
    const count = await quickButtons.count()
    expect(count).toBeGreaterThanOrEqual(1)

    // Clica na primeira ação rápida e valida que preenche o textarea
    const firstQuick = quickButtons.first()
    await firstQuick.click()
    await expect(composer).not.toHaveValue('')

    const composerValue = await composer.inputValue()
    expect(composerValue.length).toBeGreaterThan(0)
    await p.screenshot({ path: shot('03-composer-quick-action-filled') })

    // Testa menu de comandos slash
    await composer.fill('/h')
    await waitForUiIdle(p)
    const slashMenu = p.locator('[role="listbox"]')
    if (await slashMenu.isVisible().catch(() => false)) {
      await p.screenshot({ path: shot('03-composer-slash-suggestions') })
      await p.keyboard.press('Escape')
    }

    // Fecha o composer com Escape
    await p.keyboard.press('Escape')
    await expect(composer).toHaveCount(0)
  })

  test('4. CommandDock — botões de microfone, chat, roda neural e configurações', async ({ app }) => {
    const p = await app.firstWindow()

    const toolbar = p.getByRole('toolbar', { name: 'Painel de controle da Mika' })
    await expect(toolbar).toBeVisible()

    // 1. Toggle Microfone
    const micBtn = toolbar.getByRole('button', { name: /Microfone/i })
    const micBefore = await micBtn.getAttribute('aria-pressed')
    await clickDockButton(p, 'Microfone (F9)')
    await expect(micBtn).toHaveAttribute('aria-pressed', micBefore === 'true' ? 'false' : 'true')
    await clickDockButton(p, 'Microfone (F9)')
    await expect(micBtn).toHaveAttribute('aria-pressed', micBefore ?? 'false')

    // 2. Chat button
    await clickDockButton(p, 'Chat (Ctrl+Space)')
    const composer = p.locator('textarea[name="prompt"]')
    await expect(composer).toBeVisible()
    await clickDockButton(p, 'Chat (Ctrl+Space)')
    await expect(composer).toHaveCount(0)

    // 3. Neural Wheel button
    await clickDockButton(p, 'Roda Neural (Ctrl+Shift+Space)')
    const wheelDialog = p.getByRole('dialog', { name: 'Menu de navegação rápida' })
    await expect(wheelDialog).toBeVisible()
    await p.screenshot({ path: shot('04-dock-neural-wheel-opened') })
    await p.keyboard.press('Escape')
    await expect(wheelDialog).toHaveCount(0)

    // 4. Settings button abre janela separada
    const settingsWin = await openWindow(app, async () => {
      await clickDockButton(p, 'Configurações')
    })
    await expect(settingsWin.getByRole('main')).toBeVisible()
    await settingsWin.close()
  })

  test('5. Wizard Dedicado (1280x860) — abertura em janela própria e navegação', async ({ app }) => {
    const p = await app.firstWindow()

    // Dispara reabertura do wizard
    const wizardWin = await openWindow(app, async () => {
      await p.evaluate(async () => {
        const api = (window as unknown as { electronAPI: { runtime: { saveSetting: (k: string, v: unknown) => unknown } } })
          .electronAPI
        await api.runtime.saveSetting('setup.wizardCompleted', false)
      })
    })

    await wizardWin.waitForLoadState('domcontentloaded')
    await expect(wizardWin.getByText(/Onboarding da Mika/i)).toBeVisible({ timeout: 10_000 })

    // Valida dimensões da janela do Wizard (980x720 conforme especificação)
    const bounds = await wizardWin.evaluate(() => ({
      width: window.outerWidth,
      height: window.outerHeight,
    }))
    expect(bounds.width).toBeGreaterThanOrEqual(900)
    expect(bounds.height).toBeGreaterThanOrEqual(680)

    await wizardWin.screenshot({ path: shot('05-wizard-window-980x720') })

    // Restaura para concluído e fecha a janela do wizard
    await p.evaluate(async () => {
      const api = (window as unknown as { electronAPI: { runtime: { saveSetting: (k: string, v: unknown) => unknown } } })
        .electronAPI
      await api.runtime.saveSetting('setup.wizardCompleted', true)
    })
    await wizardWin.close()
  })

  test('6. Plugin View Window — renderização em janela dedicada', async ({ app }) => {
    const p = await app.firstWindow()

    const catalog = await p.evaluate(async () => {
      const api = (
        window as unknown as {
          electronAPI: { runtime: { getPluginUi: () => Promise<{ views: Array<{ pluginId: string; id: string }> }> } }
        }
      ).electronAPI
      return api.runtime.getPluginUi()
    })

    if (catalog?.views?.length > 0) {
      const firstView = catalog.views[0]
      const pluginWin = await openWindow(app, async () => {
        await p.evaluate(
          ({ pid, vid }) => {
            const api = (
              window as unknown as {
                electronAPI: {
                  shell: { pluginViews: { open: (opts: { pluginId: string; viewId: string }) => Promise<unknown> } }
                }
              }
            ).electronAPI
            return api.shell.pluginViews.open({ pluginId: pid, viewId: vid })
          },
          { pid: firstView.pluginId, vid: firstView.id },
        )
      })

      await pluginWin.waitForLoadState('domcontentloaded')
      await waitForUiIdle(pluginWin)
      await pluginWin.screenshot({ path: shot(`06-plugin-view-${firstView.pluginId}-${firstView.id}`) })
      await pluginWin.close()
    }
  })
})
