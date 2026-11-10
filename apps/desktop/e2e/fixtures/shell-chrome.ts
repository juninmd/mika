import { expect, type Page } from '@playwright/test'
import { waitForUiIdle } from './settled'

type ShellApi = {
  electronAPI: {
    runtime: {
      saveSetting(key: string, value: unknown): Promise<unknown>
      getSettingsConfig(): Promise<{ config: { character?: { backgroundMode?: string } } }>
    }
  }
}

export async function enableShellChrome(page: Page): Promise<void> {
  await page.evaluate(
    ({ k, v }: { k: string; v: unknown }) => (window as never as ShellApi).electronAPI.runtime.saveSetting(k, v),
    { k: 'character.backgroundMode', v: 'solid' },
  )

  // O que o `waitForTimeout(600)` esperava: o store ter gravado e o renderer ter repintado com o chrome.
  await expect
    .poll(() =>
      page.evaluate(async () => (await (window as never as ShellApi).electronAPI.runtime.getSettingsConfig()).config.character?.backgroundMode),
    )
    .toBe('solid')
  // O store gravou, mas o renderer só tem o chrome (e a paleta, o menu, os diálogos — irmãos na mesma
  // árvore) quando o fundo sólido aparece no DOM. Sem isto a primeira tecla corria na frente da montagem.
  await expect(page.getByTestId('stage-backdrop-fill')).toBeAttached()
  await waitForUiIdle(page)
}
