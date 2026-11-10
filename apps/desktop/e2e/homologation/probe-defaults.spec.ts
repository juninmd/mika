import { writeArtifact } from '../fixtures/artifact'
import { callE2eHook, expect, test } from '../fixtures/tauri-app'

// Lê, pelo gancho `config-defaults` (`src-tauri/src/modules/e2e_hooks.rs`), os defaults do MESMO
// `SchemaRegistry` que semeia o `ConfigStore` no primeiro boot — o registro de produção, não o de
// um teste Rust. O que se afirma aqui são os defaults que importam para a promessa do produto
// (PRIVACY.md §3: nada que sai da máquina ou controla o computador nasce ligado; o resto local
// nasce ligado). Valor esperado vem do contrato, não da própria sonda.
test('sonda: os defaults de produção chegam ao registro de config', async ({ app }) => {
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')

  const defaults = await callE2eHook<Record<string, Record<string, unknown>>>(page, 'config-defaults')

  const observed = {
    'character.enabled': defaults.character?.enabled,
    'mood.enabled': defaults.mood?.enabled,
    'person-catalog.enabled': defaults['person-catalog']?.enabled,
    'browser.enabled': defaults.browser?.enabled,
    'computer-use.enabled': defaults['computer-use']?.enabled,
    'person-catalog.webEnrichment': defaults['person-catalog']?.webEnrichment,
    'telemetry.enabled': defaults.telemetry?.enabled,
  }
  expect(observed).toEqual({
    'character.enabled': true,
    'mood.enabled': true,
    'person-catalog.enabled': true,
    'browser.enabled': false,
    'computer-use.enabled': false,
    'person-catalog.webEnrichment': false,
    'telemetry.enabled': false,
  })
  writeArtifact('probe-defaults', observed)
})
