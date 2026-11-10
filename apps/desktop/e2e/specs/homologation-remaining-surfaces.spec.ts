import { mkdirSync } from 'node:fs'
import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { openComposer } from '../fixtures/composer'
import { callE2eHook, expect, test, type TauriShellApp } from '../fixtures/tauri-app'
import { waitForSettled, waitForUiIdle } from '../fixtures/settled'
import { enableShellChrome } from '../fixtures/shell-chrome'
import { clickDockButton } from '../fixtures/dock'

const SCREENSHOTS_DIR = join(process.cwd(), 'e2e/screenshots')
function shotPath(name: string) {
  return join(SCREENSHOTS_DIR, `${name}.png`)
}

type BridgeWindow = Window & {
  electronAPI?: {
    runtime: {
      setTelemetryOptIn: (opts: { enabled: boolean; optInShown: boolean }) => Promise<unknown>
    }
  }
}

async function waitForBridge(page: Page) {
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as BridgeWindow).electronAPI))).toBe(true)
}

// A ponte responde antes de o React montar o App: o evento do hook e o clique direito chegavam
// antes de a janela principal assinar o que os ouve (aprovação de ferramenta, menu de contexto).
async function waitForAppShell(page: Page) {
  await expect(page.locator('[role="toolbar"]')).toBeAttached()
}

async function openWindow(app: TauriShellApp, openFn: () => Promise<void>): Promise<Page> {
  const winPromise = app.waitForEvent('window')
  await openFn()
  const win = await winPromise
  await win.waitForLoadState('domcontentloaded')
  return win
}

test.describe('Homologação Remaining Surfaces', () => {
  test.beforeAll(() => {
    mkdirSync(SCREENSHOTS_DIR, { recursive: true })
  })

  test('1. ToolApproval Dialog', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)
    await waitForAppShell(p)

    // Ported off `app.evaluate` (Electron main-process only) to the `trigger-tool-approval` e2e hook
    // (`src-tauri/src/modules/e2e_hooks.rs`), which calls the REAL installed
    // `WindowApprovalHandler::decide` (`modules/tools/window_approval.rs::trigger_e2e_approval_request`)
    // — the same code path a real tool call takes to open this dialog; only the tool name/id are
    // synthetic.
    await callE2eHook(p, 'trigger-tool-approval')

    await expect(p.getByRole('dialog')).toBeVisible()
    await expect(p.getByText('Confirmação Necessária')).toBeVisible()

    await expect(p.getByRole('button', { name: 'Permitir', exact: true })).toBeVisible()
    await expect(p.getByRole('button', { name: 'Permitir nesta sessão' })).toBeVisible()
    await expect(p.getByRole('button', { name: 'Permitir sempre' })).toBeVisible()
    await waitForSettled(p, '[role="dialog"]')
    await p.screenshot({ path: shotPath('overlay-tool-approval'), fullPage: true })

    await p.getByRole('button', { name: /Negar/ }).click()
  })

  test('2. Notification Toasts', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)
    await enableShellChrome(p)

    const toastText = () =>
      p.evaluate(() => document.querySelector('[data-testid="notification-toasts"]')?.textContent ?? '')
    for (const command of ['/help', '/stats']) {
      const before = await toastText()
      const input = await openComposer(p)
      await input.fill(command)
      // Primeiro Enter seleciona a sugestão no menu de autocomplete slash, segundo Enter submete
      await input.press('Enter')
      await input.press('Enter')
      // O comando foi tratado quando a coluna de toasts mudou (cada slash command emite uma notificação).
      await expect.poll(toastText).not.toBe(before)
    }

    const toasts = p.getByTestId('notification-toasts')
    await expect(toasts.locator('> *').first()).toBeVisible()
    await waitForSettled(p, '[data-testid="notification-toasts"]')
    await p.screenshot({ path: shotPath('overlay-notifications-all'), fullPage: true })
  })

  test('7. Telemetry Opt-In Dialog', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await p.evaluate(async () => {
      const api = (window as unknown as BridgeWindow).electronAPI
      await api?.runtime.setTelemetryOptIn({ enabled: false, optInShown: false })
    })
    await p.reload()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await expect(p.getByText('Privacidade primeiro')).toBeVisible()

    await expect(p.getByRole('button', { name: 'Ativar telemetria' })).toBeVisible()
    await expect(p.getByRole('button', { name: 'Manter desativada' })).toBeVisible()
    await waitForSettled(p, '[role="dialog"], .fixed.inset-0')
    await p.screenshot({ path: shotPath('overlay-telemetry-optin'), fullPage: true })

    const hit = await p.evaluate(() => {
      const button = [...document.querySelectorAll('button')].find((b) => b.textContent?.includes('Ativar telemetria'))
      if (!button) return { found: false, onTop: false, blocker: '' }
      const box = button.getBoundingClientRect()
      const top = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2)
      return {
        found: true,
        onTop: top === button || button.contains(top),
        blocker: (top as HTMLElement | null)?.className?.slice?.(0, 90) ?? '',
      }
    })
    expect(hit.found).toBe(true)
    expect(hit.onTop, `elemento por cima do CTA: ${hit.blocker}`).toBe(true)

    await p.getByRole('button', { name: 'Manter desativada' }).click()
  })

  test('9. Settings Overview Deck + Module Cards', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const settings = await openWindow(app, async () => {
      await clickDockButton(p, 'Configurações')
    })

    await settings.getByRole('button', { name: 'Sistema' }).first().click()
    await settings.getByRole('tab', { name: 'Resumo', exact: true }).click()
    await expect(settings.getByRole('tab', { name: 'Resumo', exact: true })).toHaveAttribute('aria-selected', 'true')
    await waitForUiIdle(settings)
    await settings.screenshot({ path: shotPath('settings-overview-deck'), fullPage: true })

    await settings.getByRole('tab', { name: 'Módulos', exact: true }).click()
    await expect(settings.getByRole('tab', { name: 'Módulos', exact: true })).toHaveAttribute('aria-selected', 'true')
    await waitForUiIdle(settings)
    await settings.screenshot({ path: shotPath('settings-modules-deck'), fullPage: true })

    await settings.getByRole('tab', { name: 'Diagnóstico', exact: true }).click()
    await expect(settings.getByRole('tab', { name: 'Diagnóstico', exact: true })).toHaveAttribute('aria-selected', 'true')
    await waitForUiIdle(settings)
    await settings.screenshot({ path: shotPath('settings-homologation-panel'), fullPage: true })

    await settings.close()
  })

  test('11. Chat Input Overlay', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    await openComposer(p)
    await waitForUiIdle(p)

    await p.screenshot({ path: shotPath('overlay-chat-input-v2'), fullPage: true })
    await p.keyboard.press('Escape')
  })

  test('12. Settings Sidebar Compact/Expanded', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)

    const settings = await openWindow(app, async () => {
      await clickDockButton(p, 'Configurações')
    })
    await waitForUiIdle(settings)

    const collapseBtn = settings.locator('button[title="Recolher"], button[title="Expandir"]')
    if (await collapseBtn.isVisible().catch(() => false)) {
      await collapseBtn.click()
      await waitForUiIdle(settings)
      await settings.screenshot({ path: shotPath('settings-sidebar-compact'), fullPage: true })
      await collapseBtn.click()
      await waitForUiIdle(settings)
      await settings.screenshot({ path: shotPath('settings-sidebar-expanded'), fullPage: true })
    } else {
      await settings.screenshot({ path: shotPath('settings-sidebar-default'), fullPage: true })
    }
    await settings.close()
  })

  test('13. Menu de contexto + Roda Neural', async ({ app }) => {
    const p = await app.firstWindow()
    await p.waitForLoadState('domcontentloaded')
    await waitForBridge(p)
    await waitForAppShell(p)

    await p.mouse.click(200, 320, { button: 'right' })
    await expect(p.getByRole('menu', { name: 'Ações da Mika' })).toBeVisible()
    await waitForSettled(p, '[role="menu"]')
    await p.screenshot({ path: shotPath('overlay-context-menu'), fullPage: true })

    await p.mouse.click(20, 20)
    await expect(p.getByRole('menu', { name: 'Ações da Mika' })).toBeHidden()

    await p.mouse.click(200, 320, { button: 'middle' })
    await expect(p.getByRole('dialog', { name: 'Menu de navegação rápida' })).toBeVisible()

    await waitForUiIdle(p)
    await p.screenshot({ path: shotPath('overlay-neural-wheel'), fullPage: true })

    await p.getByTestId('neural-wheel-item-games').click({ force: true })
    await expect(
      p
        .getByText('Mais jogados')
        .or(p.getByText('Jogos instalados'))
        .or(p.getByText('Steam não encontrada'))
        .or(p.getByText('Nenhum jogo encontrado na Steam'))
        .or(p.getByText('Lendo sua biblioteca…'))
        .first(),
    ).toBeVisible()
    await waitForUiIdle(p)
    await p.screenshot({ path: shotPath('overlay-neural-wheel-games'), fullPage: true })

    await p.keyboard.press('Escape')
    await expect(p.getByRole('button', { name: 'Olhar a tela' })).toBeVisible()
    // Item de comando: fecha a roda e vira um turno do chat (Mika executa e responde), em vez de
    // disparar a tool em silêncio como antes.
    await p.getByTestId('neural-wheel-item-system').click({ force: true })
    await expect(p.getByRole('dialog', { name: 'Menu de navegação rápida' })).toBeHidden()
  })
})
