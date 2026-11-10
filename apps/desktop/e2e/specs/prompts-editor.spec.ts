import { existsSync, readdirSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { discoverPromptDirEntries } from '../../build/prompt-dirs'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { expect, test } from '../fixtures/tauri-app'

type Bridge = Window & { electronAPI: ElectronShellApi }
type PromptSummary = { namespace: string; key: string }

// Covers the prompts editor (registry + IPC list/save/reset): edit, save, persist, restore, and
// proves against the filesystem: no cross-namespace contamination, recursive enumeration included.

function countMarkdownRecursive(dir: string): number {
  let count = 0
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) count += countMarkdownRecursive(join(dir, entry.name))
    else if (entry.name.endsWith('.md')) count += 1
  }
  return count
}

function expectedPromptCounts(): { total: number; byNamespace: Record<string, number> } {
  const desktopRoot = resolve(__dirname, '..', '..')
  const byNamespace: Record<string, number> = {}
  for (const { namespace, dir } of discoverPromptDirEntries(desktopRoot)) {
    const abs = resolve(desktopRoot, dir)
    if (!existsSync(abs)) continue
    byNamespace[namespace] = (byNamespace[namespace] ?? 0) + countMarkdownRecursive(abs)
  }
  const total = Object.values(byNamespace).reduce((a, b) => a + b, 0)
  return { total, byNamespace }
}

test('editor de prompts lista tudo sem contaminação, edita aninhado, persiste e restaura', async ({ app }) => {
  const expected = expectedPromptCounts()
  expect(expected.total).toBeGreaterThan(50) // sanity on the calculation itself: ~60 prompts in the tree

  const main = await app.firstWindow()
  await main.waitForLoadState('domcontentloaded')
  await expect
    .poll(() => main.evaluate(async () => (await (window as unknown as Bridge).electronAPI.runtime.getHealth()).bootstrap.pluginsReady))
    .toBe(true)

  // Boundary proof: the IPC registry must match the filesystem exactly (no contamination, no missed nesting).
  const listed = await main.evaluate(
    async () => (await (window as unknown as Bridge).electronAPI.runtime.getPrompts()).prompts,
  )
  expect(listed.length).toBe(expected.total)

  const byNamespace = new Map<string, PromptSummary[]>()
  for (const p of listed as PromptSummary[]) {
    const list = byNamespace.get(p.namespace) ?? []
    list.push(p)
    byNamespace.set(p.namespace, list)
  }
  for (const [namespace, count] of Object.entries(expected.byNamespace)) {
    expect(byNamespace.get(namespace)?.length, `namespace "${namespace}"`).toBe(count)
  }
  // Named regression: a vision prompt must not appear under "brain".
  expect(byNamespace.get('brain')?.some((p) => p.key === 'analyze-screen.prompt.md')).toBe(false)
  expect(byNamespace.get('brain')?.some((p) => p.key.startsWith('awareness/'))).toBe(true)

  const opened = app.waitForEvent('window')
  await main.evaluate(() => (window as unknown as Bridge).electronAPI.shell.openSettings())
  const settings = await opened
  await settings.waitForLoadState('domcontentloaded')

  // "Prompts" is a built-in tab, not a feature declaration, so the command palette can't reach it.
  await settings.getByRole('button', { name: 'Cognição' }).first().click()
  await settings.getByRole('tab', { name: 'Prompts' }).click()

  const searchBox = settings.getByPlaceholder('Buscar por módulo, chave ou texto…')
  await expect(searchBox).toBeVisible()

  const totalTile = settings.getByRole('status', { name: `Total de prompts: ${expected.total}` })
  await expect(totalTile).toBeVisible()

  // Edited prompt is nested (awareness/), not top-level — covers defect #2 in the UI.
  await searchBox.fill('awareness/game-absence')
  const nestedCard = settings.locator('span.font-mono', { hasText: 'awareness/game-absence.md' })
  await expect(nestedCard).toBeVisible()

  await settings.screenshot({ path: join(__dirname, '../screenshots/prompts-editor-list.png') })

  await nestedCard.click()
  const expandedCard = settings.locator('div.rounded-xl', { has: settings.locator('textarea') }).last()
  await expect(expandedCard).toHaveClass(/col-span-full/)

  const textarea = settings.locator('textarea').first()
  await expect(textarea).toBeVisible()
  const marker = `MARCADOR_PROMPT_EDITOR_731 ${Date.now()}`
  await textarea.fill(marker)
  // Waiting for the Collapsible animation to finish avoids the screenshot cutting off save/restore buttons.
  const saveButton = settings.getByRole('button', { name: 'Salvar', exact: true })
  await saveButton.scrollIntoViewIfNeeded()
  await expect(saveButton).toBeInViewport()
  await expect(saveButton).toBeEnabled()

  await settings.screenshot({ path: join(__dirname, '../screenshots/prompts-editor-edit.png') })

  await settings.getByRole('button', { name: 'Salvar', exact: true }).click()
  const customizedPill = settings.getByText('customizado', { exact: true })
  await expect(customizedPill).toBeVisible()
  await expect(settings.getByRole('status', { name: `Total de prompts: ${expected.total}` })).toBeVisible()
  await expect(settings.getByRole('status', { name: 'Customizados: 1' })).toBeVisible()

  await settings.screenshot({ path: join(__dirname, '../screenshots/prompts-editor-customized.png') })

  await settings.reload()
  await settings.waitForLoadState('domcontentloaded')
  await settings.getByRole('button', { name: 'Cognição' }).first().click()
  await settings.getByRole('tab', { name: 'Prompts' }).click()
  await settings.getByPlaceholder('Buscar por módulo, chave ou texto…').fill('awareness/game-absence')
  await expect(settings.getByText('customizado', { exact: true })).toBeVisible()
  await settings.locator('span.font-mono', { hasText: 'awareness/game-absence.md' }).click()
  await expect(settings.locator('textarea').first()).toHaveValue(marker)

  await settings.getByRole('button', { name: /Restaurar padrão/ }).click()
  await settings.getByRole('button', { name: 'Confirmar' }).click()
  await expect(settings.getByText('customizado', { exact: true })).toHaveCount(0)
})
