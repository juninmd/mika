import type { Page } from '@playwright/test'
import type { ElectronShellApi } from '../../src/shared/electron-api-types'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { expect, test } from '../fixtures/tauri-app'

type Bridge = Window & { electronAPI: ElectronShellApi }
type Fact = { id: string; fact: string }

// Painel de memória (Configurações > Memória): ver, buscar, editar e apagar o que a Mika lembra, pelo
// backend real (`memory_console` sobre o grafo social). Com o modo passageiro ligado nada pode ser
// gravado nem alterado — só lido e apagado —, e o painel tem de dizer isso em vez de falhar calado.

const tool = (page: Page, args: Record<string, unknown>) =>
  page.evaluate((a) => (window as unknown as Bridge).electronAPI.runtime.executeTool('memory_console', a), args)

async function storedFacts(page: Page): Promise<string[]> {
  const result = (await tool(page, { action: 'facts', limit: 50 })) as { data?: { facts?: Fact[] } | Fact[] }
  const data = result.data
  const facts = Array.isArray(data) ? data : (data?.facts ?? [])
  return facts.map((f) => f.fact)
}

const setPassenger = (page: Page, on: boolean) =>
  page.evaluate((v) => (window as unknown as Bridge).electronAPI.runtime.saveSetting('privacy.passengerMode', v), on)

test('painel de memória: ver, buscar, editar e apagar; modo passageiro só deixa ler e apagar', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await expect
    .poll(() => page.evaluate(async () => (await (window as unknown as Bridge).electronAPI.runtime.getHealth()).bootstrap.pluginsReady))
    .toBe(true)

  for (const fact of ['gosta de café expresso', 'joga Hades no fim de semana', 'trabalha com Rust']) {
    expect((await tool(page, { action: 'add_fact', fact })) as { success: boolean }).toMatchObject({ success: true })
  }

  const settingsPromise = app.waitForEvent('window')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')
  await settings.getByLabel('Abrir paleta de comandos').click()
  await settings.getByLabel('Buscar configurações').fill('Memória')
  await settings.keyboard.press('Enter')

  // Ver
  await expect(settings.getByText('gosta de café expresso')).toBeVisible({ timeout: 15_000 })
  await expect(settings.getByText('trabalha com Rust')).toBeVisible()

  // Buscar
  const filter = settings.getByLabel('Filtrar os fatos já exibidos')
  await filter.fill('hades')
  await expect(settings.getByText('joga Hades no fim de semana')).toBeVisible()
  await expect(settings.getByText('gosta de café expresso')).toHaveCount(0)
  await filter.fill('')

  // Editar
  await settings.getByRole('button', { name: 'Editar fato: gosta de café expresso' }).click()
  const editField = settings.getByLabel('Editar fato', { exact: true })
  await editField.fill('gosta de café coado')
  await settings.getByRole('button', { name: 'Salvar fato' }).click()
  await expect(settings.getByText('gosta de café coado')).toBeVisible()
  await expect.poll(() => storedFacts(page)).toContain('gosta de café coado')
  expect(await storedFacts(page)).not.toContain('gosta de café expresso')
  await settings.screenshot({ path: artifactPath('memory-panel-edited.png') })

  // Apagar
  await settings.getByRole('button', { name: 'Apagar fato: trabalha com Rust', exact: true }).click()
  await settings.getByRole('button', { name: 'Apagar: Apagar fato: trabalha com Rust' }).click()
  await expect(settings.getByText('trabalha com Rust')).toHaveCount(0)
  await expect.poll(() => storedFacts(page)).not.toContain('trabalha com Rust')

  // Modo passageiro: aviso visível, gravar/editar travados na UI e no backend, apagar segue liberado.
  await setPassenger(page, true)
  const notice = settings.getByText(/Modo passageiro ligado/)
  await expect(notice).toBeVisible()
  await expect(settings.getByRole('button', { name: 'Adicionar', exact: true })).toBeDisabled()
  await expect(settings.getByRole('button', { name: 'Editar fato: gosta de café coado' })).toBeDisabled()
  const blocked = (await tool(page, { action: 'add_fact', fact: 'não pode gravar' })) as { success: boolean }
  expect(blocked.success).toBe(false)
  await settings.screenshot({ path: artifactPath('memory-panel-passenger.png') })

  await settings.getByRole('button', { name: 'Apagar fato: joga Hades no fim de semana', exact: true }).click()
  await settings.getByRole('button', { name: 'Apagar: Apagar fato: joga Hades no fim de semana' }).click()
  await expect.poll(() => storedFacts(page)).not.toContain('joga Hades no fim de semana')

  await setPassenger(page, false)
  await expect(notice).toHaveCount(0)
  await expect(settings.getByRole('button', { name: 'Editar fato: gosta de café coado' })).toBeEnabled()

  const finalFacts = await storedFacts(page)
  expect(finalFacts).toEqual(['gosta de café coado'])
  writeArtifact('memory-panel', { finalFacts, passengerAddBlocked: !blocked.success })
})
