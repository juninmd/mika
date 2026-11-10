import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { Locator, Page } from '@playwright/test'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { expect, test, type TauriShellApp } from '../fixtures/tauri-app'
import { clickDockButton } from '../fixtures/dock'
import { waitForUiIdle } from '../fixtures/settled'

// Controls audit — E2E proof of the "zero dead button" invariant (AGENTS.md §3.2). Every switch/checkbox/
// input/select/button in <main> must show observable response, or the test fails naming the control.

const OUT_DIR = join(process.cwd(), 'e2e/screenshots/controls-audit')

// Skipped: exit navigation and native OS dialogs (e.g. file export) that can't be automated. The test
// runs on an isolated throwaway profile, so destructive/clear/reset/delete buttons MUST run to completion.
const SKIP_BUTTON_RE = /^(sair|voltar|fechar tudo|logout|deslogar|sair da conta|exportar)$/i

function shouldSkipButton(name: string): boolean {
  return SKIP_BUTTON_RE.test(name)
}

type ControlResult = {
  kind: 'switch' | 'checkbox' | 'input' | 'textarea' | 'range' | 'select' | 'radio-group' | 'button'
  name: string
  status: string
}

type TabReport = { tab: string; controls: ControlResult[] }
type AuditReport = { runAt: string; tabs: TabReport[]; consoleErrors?: string[] }

type BridgeWindow = Window & { electronAPI?: ElectronShellApi }

async function waitForBridge(page: Page): Promise<void> {
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as BridgeWindow).electronAPI))).toBe(true)
}

// Os `waitForTimeout(150..400)` que esperavam "o save assentar" viraram uma barreira determinística:
// uma leitura da config pelo MESMO canal IPC que os saves usam (a gravação pendente já terminou
// quando ela responde) + animações terminadas e dois quadros pintados.
async function settlePanel(settings: Page): Promise<void> {
  await settings.evaluate(() => (window as unknown as BridgeWindow).electronAPI!.runtime.getSettingsConfig())
  await waitForUiIdle(settings)
}

async function openWindow(app: TauriShellApp, openFn: () => Promise<void>): Promise<Page> {
  const winPromise = app.waitForEvent('window')
  await openFn()
  const win = await winPromise
  await win.waitForLoadState('domcontentloaded')
  return win
}

async function getSettingsDeclarations(page: Page): Promise<Array<{ tabId: string; tabLabel: string }>> {
  const { declarations } = await page.evaluate(() =>
    (window as unknown as BridgeWindow).electronAPI!.runtime.getSettingsDeclarations(),
  )
  const tabs: Array<{ tabId: string; tabLabel: string }> = []
  const seen = new Set<string>()
  for (const decl of declarations) {
    if (!decl.tabId || seen.has(decl.tabId)) continue
    seen.add(decl.tabId)
    tabs.push({ tabId: decl.tabId, tabLabel: decl.tabLabel })
  }
  return tabs
}

async function navigateSettingsTab(settings: Page, query: string): Promise<void> {
  await settings.getByLabel('Abrir paleta de comandos').click()
  await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
  await settings.getByLabel('Buscar configurações').fill(query)
  await settings.keyboard.press('Enter')
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)
  await expect(settings.getByRole('heading').first()).toBeVisible()
}

// Built-in tabs aren't in feature declarations, so the command palette can't reach them; navigate via nav rail + sub-tab.
const BUILT_IN_TABS: Array<{ section: string; label: string }> = [
  { section: 'Sistema', label: 'Módulos' },
  { section: 'Sistema', label: 'Resumo' },
  { section: 'Sistema', label: 'Diagnóstico' },
  { section: 'Sistema', label: 'Logs' },
  { section: 'Sistema', label: 'Atualizações' },
  { section: 'Mika', label: 'Skin' },
  { section: 'Ferramentas', label: 'Catálogo' },
  { section: 'Conta', label: 'Credenciais' },
  { section: 'Voz', label: 'Dispositivos de Áudio' },
]

async function navigateBuiltInTab(settings: Page, sectionLabel: string, tabLabel: string): Promise<void> {
  const nav = settings.getByRole('navigation', { name: /configura/i })
  await nav.getByRole('button', { name: sectionLabel, exact: true }).click()
  const tab = settings.locator('[role="tablist"]').getByRole('tab', { name: tabLabel, exact: true }).first()
  await expect(tab).toBeVisible()
  await tab.click()
  await expect(tab).toHaveAttribute('aria-selected', 'true')
  await waitForUiIdle(settings)
  await expect(settings.getByText('Manual panel required')).toHaveCount(0)
}

function slug(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/gi, '-').replace(/^-+|-+$/g, '') || 'tab'
}

async function countVisible(locator: Locator): Promise<number> {
  const all = await locator.all()
  let n = 0
  for (const item of all) {
    if (await item.isVisible().catch(() => false)) n += 1
  }
  return n
}

async function controlName(locator: Locator, fallback: string): Promise<string> {
  const label = await locator.getAttribute('aria-label').catch(() => null)
  if (label) return label
  const text = (await locator.textContent().catch(() => '')) ?? ''
  return text.trim() || fallback
}

async function auditSwitch(_settings: Page, sw: Locator): Promise<ControlResult> {
  const name = await controlName(sw, 'switch')
  if (await sw.isDisabled().catch(() => true)) return { kind: 'switch', name, status: 'disabled' }
  const before = await sw.getAttribute('aria-checked')
  await sw.click()
  await expect(sw).toHaveAttribute('aria-checked', before === 'true' ? 'false' : 'true')
  await expect(sw).toBeEnabled({ timeout: 10_000 })
  await sw.click()
  await expect(sw).toHaveAttribute('aria-checked', before ?? 'false')
  return { kind: 'switch', name, status: 'flipped' }
}

async function auditCheckbox(_settings: Page, cb: Locator): Promise<ControlResult> {
  const name = await controlName(cb, 'checkbox')
  if (await cb.isDisabled().catch(() => true)) return { kind: 'checkbox', name, status: 'disabled' }
  const wasChecked = await cb.isChecked()
  // A primeira virada PROVA a resposta e falha de verdade se o controle não reage (antes o try/catch
  // abraçava as duas viradas e um checkbox morto saía como 'toggled').
  if (wasChecked) {
    await cb.uncheck({ timeout: 5000 })
    await expect(cb).not.toBeChecked()
  } else {
    await cb.check({ timeout: 5000 })
    await expect(cb).toBeChecked()
  }
  try {
    await expect(cb).toBeEnabled({ timeout: 10_000 })
    if (wasChecked) await cb.check({ timeout: 5000 })
    else await cb.uncheck({ timeout: 5000 })
  } catch {
    // Só a RESTAURAÇÃO é melhor-esforço: o card do módulo re-renderiza durante o save e pode perder o alvo.
  }
  return { kind: 'checkbox', name, status: 'toggled' }
}

function pickTestValue(type: string | null, min: number | null, max: number | null): string {
  if (type === 'number') {
    const base = Math.max(1, min ?? 1)
    const value = max !== null && base > max ? max : base
    return String(value)
  }
  if (type === 'url') return 'https://e2e-audit.example.com'
  if (type === 'email') return 'e2e.audit@example.com'
  if (type === 'password') return 'e2e-audit-senha'
  if (type === 'search') return 'auditoria'
  return `E2E audit ${Date.now()}`
}

async function inputName(input: Locator): Promise<string> {
  const label = await input.getAttribute('aria-label').catch(() => null)
  if (label) return label
  const associated = await input
    .evaluate((el) => {
      const id = el.id
      if (id) {
        const lbl = document.querySelector(`label[for="${id}"]`)
        const text = lbl?.textContent?.trim()
        if (text) return text
      }
      const labelledBy = el.getAttribute('aria-labelledby')
      if (labelledBy) {
        const ref = document.getElementById(labelledBy)
        const text = ref?.textContent?.trim()
        if (text) return text
      }
      return ''
    })
    .catch(() => '')
  return associated || 'input'
}

async function auditTextLike(settings: Page, input: Locator, kind: 'input' | 'textarea'): Promise<ControlResult> {
  const name = await inputName(input)
  if (await input.isDisabled().catch(() => true)) return { kind, name, status: 'disabled' }
  const type = await input.getAttribute('type')
  const original = await input.inputValue()
  const minRaw = await input.getAttribute('min')
  const maxRaw = await input.getAttribute('max')
  const min = minRaw === null ? null : Number(minRaw)
  const max = maxRaw === null ? null : Number(maxRaw)
  const testValue = pickTestValue(type, min, max)

  const alertsBefore = await settings.getByRole('alert').count()
  await input.fill(testValue)
  await expect(input).toHaveValue(testValue)
  await input.press('Tab')
  await settlePanel(settings)

  const accepted = (await input.inputValue()) === testValue
  const alerted = (await settings.getByRole('alert').count()) > alertsBefore

  await input.fill(original)
  await input.press('Tab')
  await settlePanel(settings)

  // An alert after blur means the field validated and rejected the value — correct response, not a dead field.
  if (alerted) return { kind, name, status: 'validated' }
  if (!accepted) return { kind, name, status: 'no-response' }
  return { kind, name, status: 'filled' }
}

async function auditRange(range: Locator): Promise<ControlResult> {
  const name = await controlName(range, 'range')
  if (await range.isDisabled().catch(() => true)) return { kind: 'range', name, status: 'disabled' }
  const min = Number(await range.getAttribute('min')) || 0
  const max = Number(await range.getAttribute('max')) || 100
  if (min === max) return { kind: 'range', name, status: 'no-range' }
  const before = await range.inputValue()
  const direction = before < String(max) ? 'ArrowRight' : 'ArrowLeft'
  await range.focus()
  await range.press(direction)
  const moved = await expect
    .poll(() => range.inputValue(), { timeout: 3000 })
    .not.toBe(before)
    .then(
      () => true,
      () => false,
    )
  await range.press(direction === 'ArrowRight' ? 'ArrowLeft' : 'ArrowRight')
  if (!moved) return { kind: 'range', name, status: 'no-response' }
  return { kind: 'range', name, status: 'moved' }
}

async function auditSelect(settings: Page, sel: Locator): Promise<ControlResult> {
  const name = await controlName(sel, 'select')
  if (await sel.isDisabled().catch(() => true)) return { kind: 'select', name, status: 'disabled' }

  await sel.click()
  await expect(settings.getByRole('listbox')).toBeVisible()
  const options = settings.getByRole('option')
  const count = await options.count()
  let originalIndex = -1
  for (let i = 0; i < count; i += 1) {
    if ((await options.nth(i).getAttribute('aria-selected')) === 'true') {
      originalIndex = i
      break
    }
  }
  await settings.keyboard.press('Escape')
  await expect(settings.getByRole('listbox')).toHaveCount(0)
  const beforeText = ((await sel.textContent()) ?? '').trim()

  await sel.click()
  await expect(settings.getByRole('listbox')).toBeVisible()
  let chosen = false
  for (let i = 0; i < count; i += 1) {
    const option = options.nth(i)
    if ((await option.getAttribute('aria-selected')) !== 'true') {
      await option.click()
      chosen = true
      break
    }
  }
  if (!chosen) {
    await settings.keyboard.press('Escape')
    await expect(settings.getByRole('listbox')).toHaveCount(0)
    return { kind: 'select', name, status: 'single-option' }
  }
  await expect(settings.getByRole('listbox')).toHaveCount(0)
  await expect
    .poll(async () => ((await sel.textContent()) ?? '').trim(), { timeout: 5000 })
    .not.toBe(beforeText)

  if (originalIndex >= 0) {
    await sel.click()
    await expect(settings.getByRole('listbox')).toBeVisible()
    await options.nth(originalIndex).click()
    await expect(settings.getByRole('listbox')).toHaveCount(0)
  }
  return { kind: 'select', name, status: 'selected' }
}

// Selects de até 6 opções viram cards (GlassChoiceCards: [role=radiogroup] > [role=radio]) e deixam de
// ser `button[aria-haspopup=listbox]` — sem este passo, cada select convertido sairia da auditoria calado.
async function checkedRadioIndex(radios: Locator): Promise<number> {
  const states = await radios.evaluateAll((els) => els.map((el) => el.getAttribute('aria-checked') === 'true'))
  return states.indexOf(true)
}

async function auditRadioGroup(settings: Page, group: Locator): Promise<ControlResult> {
  const name = (await group.getAttribute('aria-label')) || (await inputName(group))
  const radios = group.getByRole('radio')
  const count = await radios.count()
  if ((await group.getAttribute('aria-disabled')) === 'true') return { kind: 'radio-group', name, status: 'disabled' }
  if (count < 2) return { kind: 'radio-group', name, status: 'single-option' }
  const original = await checkedRadioIndex(radios)
  const target = original === 0 ? 1 : 0
  await radios.nth(target).click()
  await expect(radios.nth(target)).toHaveAttribute('aria-checked', 'true', { timeout: 5000 })

  // Teclado de radiogroup: a seta move a seleção (não só o foco) e dá a volta no fim.
  await radios.nth(target).focus()
  await settings.keyboard.press('ArrowRight')
  const next = (target + 1) % count
  await expect(radios.nth(next)).toHaveAttribute('aria-checked', 'true', { timeout: 5000 })

  if (original >= 0) {
    await radios.nth(original).click()
    await expect(radios.nth(original)).toHaveAttribute('aria-checked', 'true', { timeout: 5000 })
  }
  return { kind: 'radio-group', name, status: 'selected' }
}

type ButtonMeta = { index: number; label: string; text: string }

const TOP_BAR_NAMES = ['Abrir paleta de comandos', 'Fechar']

async function collectButtons(main: Locator): Promise<ButtonMeta[]> {
  const metas = await main.getByRole('button').evaluateAll(
    (els, topBarNames: string[]) =>
      els.map((el, index) => {
        const role = el.getAttribute('role')
        const hasPopup = el.getAttribute('aria-haspopup')
        const label = el.getAttribute('aria-label') ?? ''
        const text = (el.textContent ?? '').trim()
        const disabled = (el as HTMLButtonElement).disabled
        const inNav = Boolean(el.closest('[role="tablist"], nav, [role="navigation"], [role="toolbar"]'))
        const inConfirmGroup = Boolean(el.closest('[role="group"][aria-label^="Confirmação:"]'))
        const isTopBar = topBarNames.includes(label)
        const eligible = !role && !hasPopup && !disabled && !inNav && !inConfirmGroup && !isTopBar
        return eligible ? { index, label, text } : null
      }),
    TOP_BAR_NAMES,
  )
  return metas.filter((m): m is ButtonMeta => m !== null)
}

async function auditPanelButtons(settings: Page): Promise<ControlResult[]> {
  const main = settings.getByRole('main')
  const results: ControlResult[] = []
  const done = new Set<string>()

  // One button per iteration, re-enumerated from scratch each click: a navigating/re-rendering click
  // would orphan previously collected locators and click the wrong elements.
  for (let iter = 0; iter < 500; iter += 1) {
    const heading = ((await main.getByRole('heading').first().textContent().catch(() => '')) ?? '').trim()
    const metas = await collectButtons(main)

    let targetIndex = -1
    const skipped = new Set<string>()
    for (const meta of metas) {
      const name = meta.label || meta.text
      if (shouldSkipButton(name)) {
        const key = `${heading}|${name}`
        if (!skipped.has(key)) {
          skipped.add(key)
          results.push({ kind: 'button', name, status: 'destructive-skipped' })
        }
        continue
      }
      const key = `${heading}|${name}`
      if (done.has(key)) continue
      targetIndex = meta.index
      done.add(key)
      break
    }
    if (targetIndex < 0) return results

    const target = main.getByRole('button').nth(targetIndex)
    const name = (metas.find((m) => m.index === targetIndex)?.label ?? '') || 'button'
    try {
      await target.click({ timeout: 3000 })
    } catch {
      // Element disappeared between enumeration and click (panel re-rendered): record and continue.
      results.push({ kind: 'button', name, status: 're-rendered' })
      continue
    }
    await settlePanel(settings)

    const dialogsAfter = await countVisible(settings.getByRole('dialog'))
    if (dialogsAfter > 0) {
      const dialog = settings.getByRole('dialog').filter({ visible: true }).first()
      await expect(dialog).toBeVisible()
      const closeBtn = dialog.getByRole('button', { name: /fechar|cancelar|close|ok|entendi/i }).first()
      if (await closeBtn.isVisible().catch(() => false)) {
        await closeBtn.click()
        await waitForUiIdle(settings)
      }
      results.push({ kind: 'button', name, status: 'opened-dialog' })
    } else {
      const confirmGroup = settings.getByRole('group', { name: /^Confirmação:/ }).first()
      if (await confirmGroup.isVisible().catch(() => false)) {
        // Armed ConfirmButton: test the real confirm click (isolated test profile).
        const confirmBtn = confirmGroup
          .getByRole('button', { name: /confirmar|sim|ok|excluir|limpar|apagar|deletar|remover|zerar/i })
          .first()
        if (await confirmBtn.isVisible().catch(() => false)) {
          await confirmBtn.click().catch(() => {})
          await settlePanel(settings)
          results.push({ kind: 'button', name, status: 'confirmed-destructive' })
        } else {
          const cancelBtn = confirmGroup.getByRole('button', { name: 'Cancelar', exact: true }).first()
          await cancelBtn.click().catch(() => {})
          await waitForUiIdle(settings)
          results.push({ kind: 'button', name, status: 'armed-cancelled' })
        }
      } else {
        results.push({ kind: 'button', name, status: 'clicked' })
      }
    }

    const nextHeading = ((await main.getByRole('heading').first().textContent().catch(() => '')) ?? '').trim()
    if (nextHeading !== heading) return results
  }
  return results
}

async function auditPanel(settings: Page): Promise<ControlResult[]> {
  const main = settings.getByRole('main')
  const results: ControlResult[] = []

  const switches = await main.getByRole('switch').all()
  for (const sw of switches) results.push(await auditSwitch(settings, sw))

  const checkboxes = await main.locator('input[type="checkbox"]').all()
  for (const cb of checkboxes) results.push(await auditCheckbox(settings, cb))

  const ranges = await main.locator('input[type="range"]').all()
  for (const range of ranges) results.push(await auditRange(range))

  const textSelector =
    'input:not([type]), input[type="text"], input[type="number"], input[type="url"], input[type="email"], input[type="password"], input[type="tel"], input[type="search"]'
  const inputs = await main.locator(textSelector).all()
  for (const input of inputs) results.push(await auditTextLike(settings, input, 'input'))

  const textareas = await main.locator('textarea').all()
  for (const textarea of textareas) results.push(await auditTextLike(settings, textarea, 'textarea'))

  const selects = await main.locator('button[aria-haspopup="listbox"]').all()
  for (const sel of selects) results.push(await auditSelect(settings, sel))

  const radioGroups = await main.getByRole('radiogroup').all()
  for (const group of radioGroups) {
    if (!(await group.isVisible().catch(() => false))) continue
    try {
      results.push(await auditRadioGroup(settings, group))
    } catch {
      results.push({ kind: 'radio-group', name: await inputName(group), status: 'no-response' })
    }
  }

  results.push(...(await auditPanelButtons(settings)))

  return results
}

test.describe('Auditoria de controles', () => {
  test.beforeAll(() => {
    mkdirSync(OUT_DIR, { recursive: true })
  })

  test('janela de configurações — cada switch/checkbox/input/select/radio/botão responde', async ({ app }) => {
    test.setTimeout(900_000)

    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await waitForBridge(page)

    const settings = await openWindow(app, async () => {
      await clickDockButton(page, 'Configurações')
    })

    const tabs = await getSettingsDeclarations(page)
    expect(tabs.length).toBeGreaterThan(0)

    const pageErrors: string[] = []
    const consoleErrors: string[] = []
    settings.on('pageerror', (error) => pageErrors.push(`pageerror: ${String(error)}`))
    settings.on('console', (message) => {
      if (message.type() === 'error') consoleErrors.push(`console: ${message.text()}`)
    })

    const report: AuditReport = { runAt: new Date().toISOString(), tabs: [] }

    const auditOneTab = async (tabLabel: string, navigate: () => Promise<void>): Promise<void> => {
      console.log(`[audit] navegando para aba: ${tabLabel}`)
      try {
        await navigate()
        await settlePanel(settings)
        const controls = await auditPanel(settings)
        report.tabs.push({ tab: tabLabel, controls })
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error)
        report.tabs.push({
          tab: tabLabel,
          controls: [{ kind: 'button', name: `FALHA: ${message.slice(0, 140)}`, status: 'error' }],
        })
        console.log(`[audit] ERRO na aba ${tabLabel}: ${message.slice(0, 300)}`)
        await settings
          .screenshot({ path: join(OUT_DIR, `tab-${slug(tabLabel)}-error.png`), fullPage: true })
          .catch(() => {})
      }
      writeFileSync(join(OUT_DIR, 'report-partial.json'), JSON.stringify(report, null, 2))
      await settings.screenshot({ path: join(OUT_DIR, `tab-${slug(tabLabel)}.png`), fullPage: true }).catch(() => {})
    }

    for (const { tabLabel } of tabs) {
      await auditOneTab(tabLabel, () => navigateSettingsTab(settings, tabLabel))
    }
    for (const builtIn of BUILT_IN_TABS) {
      await auditOneTab(builtIn.label, () => navigateBuiltInTab(settings, builtIn.section, builtIn.label))
    }

    await settings.close()

    report.consoleErrors = consoleErrors
    writeFileSync(join(OUT_DIR, 'report.json'), JSON.stringify(report, null, 2))

    const failed: Array<{ tab: string; control: ControlResult }> = []
    for (const tab of report.tabs) {
      for (const control of tab.controls) {
        if (control.status === 'no-response' || control.status === 'error') {
          failed.push({ tab: tab.tab, control })
        }
      }
    }

    expect(pageErrors, `exceções não tratadas durante a auditoria:\n${pageErrors.join('\n')}`).toEqual([])
    expect(failed, `controles sem resposta:\n${JSON.stringify(failed, null, 2)}`).toEqual([])
  })

  test('janela principal — dock, paleta de comandos e composer respondem', async ({ app }) => {
    test.setTimeout(120_000)

    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await waitForBridge(page)

    const pageErrors: string[] = []
    page.on('pageerror', (error) => pageErrors.push(`pageerror: ${String(error)}`))

    const toolbar = page.getByRole('toolbar', { name: 'Painel de controle da Mika' })
    await expect(toolbar).toBeVisible()

    const mic = toolbar.getByRole('button', { name: /Microfone/ })
    const micBefore = await mic.getAttribute('aria-pressed')
    await clickDockButton(page, 'Microfone (F9)')
    await expect(mic).toHaveAttribute('aria-pressed', micBefore === 'true' ? 'false' : 'true')
    await clickDockButton(page, 'Microfone (F9)')
    await expect(mic).toHaveAttribute('aria-pressed', micBefore ?? 'false')

    await clickDockButton(page, 'Chat (Ctrl+Space)')
    const composer = page.locator('textarea[name="prompt"]')
    await expect(composer).toBeVisible()
    await composer.fill('auditoria de controles')
    await expect(composer).toHaveValue('auditoria de controles')
    await composer.fill('')
    await clickDockButton(page, 'Chat (Ctrl+Space)')
    await expect(composer).toHaveCount(0)

    await clickDockButton(page, 'Roda Neural (Ctrl+Shift+Space)')
    await waitForUiIdle(page)

    await page.keyboard.press('Control+k')
    await expect(page.getByPlaceholder('Buscar comandos e ferramentas…')).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(page.getByPlaceholder('Buscar comandos e ferramentas…')).toHaveCount(0)

    const settingsWindow = await openWindow(app, async () => {
      await clickDockButton(page, 'Configurações')
    })
    await expect(settingsWindow.getByRole('main')).toBeVisible()
    await settingsWindow.close()

    await page.screenshot({ path: join(OUT_DIR, 'main-window.png'), fullPage: true })

    expect(pageErrors, `exceções não tratadas na janela principal:\n${pageErrors.join('\n')}`).toEqual([])
  })
})