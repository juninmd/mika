import type { Page } from '@playwright/test'
import { expect, test } from '../fixtures/tauri-app'

type DebugBridge = {
  listActions: () => string[]
  playAction: (name: string) => boolean
  freeze: (name: string | null, seconds: number) => Promise<boolean>
  readParams: (ids: string[]) => Record<string, number>
  resume: () => void
}

type BridgeWindow = Window & { __mikaLive2D?: DebugBridge }

const listActions = (page: Page) => page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.listActions() ?? [])

async function waitForAvatar(page: Page): Promise<void> {
  await page.waitForLoadState('domcontentloaded')
  await expect.poll(async () => (await listActions(page)).length, { timeout: 30_000 }).toBeGreaterThan(0)
}

// O default do app e o VRM; teste de Live2D pede Live2D em vez de torcer para o default ser um.
test.use({ skin: 'haru' })

test.describe('avatar Live2D — ações', () => {
  test('a skin padrão expõe o catálogo completo de ações', async ({ app }) => {
    const page = await app.firstWindow()
    await waitForAvatar(page)

    expect((await listActions(page)).sort()).toEqual([
      'angry',
      'cheer',
      'happy',
      'laugh',
      'nod',
      'pout',
      'sad',
      'shakehead',
      'shrug',
      'shy',
      'sleepy',
      'surprised',
      'thinking',
      'wave',
      'wink',
    ])
  })

  const BODY_ACTIONS = ['wave', 'shrug', 'cheer'] as const

  const ARM_PARAMS = ['ParamArmRB'] as const

  for (const action of BODY_ACTIONS) {
    test(`a ação de corpo '${action}' tira o braço do repouso`, async ({ app }) => {
      const page = await app.firstWindow()
      await waitForAvatar(page)

      const read = () =>
        page.evaluate((ids) => (window as BridgeWindow).__mikaLive2D?.readParams([...ids]) ?? {}, ARM_PARAMS)

      const rest = await read()

      expect(rest.ParamArmRB, 'o braço já sai do repouso sem ação nenhuma').toBeCloseTo(0, 1)

      const played = await page.evaluate(
        (name) => (window as BridgeWindow).__mikaLive2D?.playAction(name) ?? false,
        action,
      )
      expect(played, `a skin padrão precisa ter a ação '${action}'`).toBe(true)

      await expect
        .poll(async () => (await read()).ParamArmRB, { message: `'${action}' não moveu o braço`, timeout: 5000 })
        .toBeGreaterThan(0.5)
    })
  }

  test('congelar uma ação move o avatar para uma pose reprodutível', async ({ app }, testInfo) => {
    const page = await app.firstWindow()
    await waitForAvatar(page)

    const frozen = await page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.freeze('thinking', 1.2) ?? false)
    expect(frozen, "a skin padrão precisa ter a ação 'thinking'").toBe(true)

    await page.screenshot({ path: testInfo.outputPath('live2d-thinking-1200ms.png') })

    await page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.resume())
  })

  const PROCEDURAL_PARAMS = [
    'ParamAngleX',
    'ParamAngleY',
    'ParamAngleZ',
    'ParamBodyAngleX',
    'ParamEyeBallX',
    'ParamEyeBallY',
    'ParamEyeLOpen',
    'ParamBreath',
  ] as const

  test('a mesma ação no mesmo instante congela na mesma pose', async ({ app }) => {
    const page = await app.firstWindow()
    await waitForAvatar(page)

    const capture = async () => {
      await page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.resume())
      await page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.freeze('surprised', 0.8))
      return page.evaluate(
        (ids) => (window as BridgeWindow).__mikaLive2D?.readParams([...ids]) ?? {},
        PROCEDURAL_PARAMS,
      )
    }

    const first = await capture()
    const second = await capture()

    expect(Object.keys(first).length, 'a ponte não devolveu nenhum parâmetro').toBeGreaterThan(0)

    expect(
      Object.values(first).some((value) => Math.abs(value) > 0.001),
      'pose toda zerada',
    ).toBe(true)
    expect(second).toEqual(first)

    await page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.resume())
  })
})
