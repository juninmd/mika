import { readFile, writeFile } from 'node:fs/promises'
import { artifactPath, writeArtifact } from '../fixtures/artifact'
import { clickDockButton } from '../fixtures/dock'
import { expect, test } from '../fixtures/tauri-app'

// Backup em TOML de ponta a ponta: exporta pelo botão real, confere o arquivo baixado (tabelas
// `[fields.<ns>]`, sem credencial quando o toggle está desligado), edita UM valor à mão — o motivo de
// o formato ser TOML — e reimporta pela config store real, que precisa refletir a edição.
test('Backup exporta e reimporta configurações em TOML', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const settingsPromise = app.waitForEvent('window')
  await clickDockButton(page, 'Configurações')
  const settings = await settingsPromise
  await settings.waitForLoadState('domcontentloaded')

  await settings.getByLabel('Abrir paleta de comandos').click()
  await settings.getByLabel('Buscar configurações').fill('backup')
  await settings.keyboard.press('Enter')
  await expect(settings.getByRole('button', { name: 'Exportar configurações' })).toBeVisible()
  await settings.screenshot({ path: artifactPath('backup-toml-panel.png') })

  const downloadPromise = settings.waitForEvent('download')
  await settings.getByRole('button', { name: 'Exportar configurações' }).click()
  const download = await downloadPromise
  expect(download.suggestedFilename()).toMatch(/^mika-settings-\d{4}-\d{2}-\d{2}\.toml$/)
  const exported = artifactPath(download.suggestedFilename())
  await download.saveAs(exported)
  const text = await readFile(exported, 'utf8')
  expect(text).toMatch(/^includesSecrets = false$/m)
  expect(text).toMatch(/^\[fields\.\S+\]$/m)

  // Edição à mão: o apelido do usuário, na tabela [fields.user].
  const edited = artifactPath('mika-settings-edited.toml')
  const editedText = text.replace(/(\[fields\.user\][^[]*?^nickname = )"[^"]*"/m, '$1"Backup TOML"')
  expect(editedText).not.toBe(text)
  await writeFile(edited, editedText)

  await settings.locator('input[type="file"]').setInputFiles(edited)
  const apply = settings.getByRole('button', { name: /^Aplicar configurações importadas/ })
  await expect(apply).toBeVisible()
  await apply.click()
  await settings.getByRole('button', { name: 'Confirmar' }).click()
  await expect(settings.getByRole('button', { name: 'Escolher arquivo…' })).toBeVisible()
  await expect
    .poll(() =>
      settings.evaluate(async () => {
        type Api = { runtime: { getSettingsConfig: () => Promise<{ config?: { user?: { nickname?: string } } }> } }
        const api = (window as unknown as { electronAPI?: Api }).electronAPI
        return (await api?.runtime.getSettingsConfig())?.config?.user?.nickname
      }),
    )
    .toBe('Backup TOML')
  await settings.screenshot({ path: artifactPath('backup-toml-imported.png') })

  writeArtifact('backup-toml', {
    filename: download.suggestedFilename(),
    tables: text.match(/^\[fields\.\S+\]$/gm)?.length ?? 0,
    includesSecrets: false,
    editedField: 'user.nickname',
    reimported: 'Backup TOML',
  })
})
