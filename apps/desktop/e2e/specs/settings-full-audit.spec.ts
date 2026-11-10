import { expect, test } from '../fixtures/tauri-app'
import { writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'

test('all registered settings panels render without Manual panel required', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const settingsPromise = app.waitForEvent('window')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')

  const { declarations } = await page.evaluate(() =>
    (
      window as unknown as { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }
    ).electronAPI.runtime.getSettingsDeclarations(),
  )

  expect(declarations.length).toBeGreaterThan(0)

  const tabsWithComponents: Array<{ tabId: string; tabLabel: string; component: string }> = []

  for (const decl of declarations) {
    if (decl.component) {
      tabsWithComponents.push({ tabId: decl.tabId, tabLabel: decl.tabLabel, component: decl.component })
    }
    for (const group of decl.groups) {
      if (group.component) {
        tabsWithComponents.push({ tabId: decl.tabId, tabLabel: decl.tabLabel, component: group.component })
      }
    }
  }

  const visitedTabs = new Set<string>()
  const uniqueTabs = tabsWithComponents.filter((t) => {
    if (visitedTabs.has(t.tabId)) return false
    visitedTabs.add(t.tabId)
    return true
  })

  expect(uniqueTabs.length).toBeGreaterThan(0)

  for (const { tabLabel } of uniqueTabs) {
    await settings.getByLabel('Abrir paleta de comandos').click()
    await expect(settings.getByLabel('Buscar configurações')).toBeVisible()
    await settings.getByLabel('Buscar configurações').fill(tabLabel)
    await settings.keyboard.press('Enter')

    await expect(settings.getByText('Manual panel required')).toHaveCount(0)

    await expect(settings.getByRole('heading').first()).toBeVisible()
  }
  writeArtifact('settings-panels-rendered', {
    declarations: declarations.length,
    tabsVisited: uniqueTabs.map((tab) => tab.tabLabel),
  })
})
