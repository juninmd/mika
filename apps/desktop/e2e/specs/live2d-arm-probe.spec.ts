import type { Page } from '@playwright/test'
import { expect, test } from '../fixtures/tauri-app'

interface DebugBridge {
  freeze(name: string | null, seconds: number): Promise<boolean>
  playAction(name: string): boolean
  probe(overrides: Record<string, number> | null): void
  readParams(ids: string[]): Record<string, number>
  resume(): void
}
type BridgeWindow = Window & { __mikaLive2D?: DebugBridge }

const listActions = (page: Page) => page.evaluate(() => ((window as BridgeWindow).__mikaLive2D ? true : false))

const skin = process.env.MIKA_E2E_SKIN ?? 'haru'

test.use({ skin })

const BODY_CHANNELS = [
  'ParamArmLA',
  'ParamArmRA',
  'ParamArmLB',
  'ParamArmRB',
  'ParamArmL',
  'ParamArmR',
  'ParamArmL01',
  'ParamArmR01',
  'ParamArmL02',
  'ParamArmR02',
  'ParamArmL03',
  'ParamArmR03',
  'ParamArmLA01',
  'ParamArmRA01',
  'ParamArmLA02',
  'ParamArmRA02',
  'ParamArmLA03',
  'ParamArmRA03',
  'ParamArmLB01',
  'ParamArmRB01',
  'ParamHandLB',
  'ParamHandRB',
  'ParamHandAngleL',
  'ParamHandAngleR',
  'ParamShoulder',
  'ParamLeftShoulderUp',
  'ParamRightShoulderUp',
  'ParamShoulderL',
  'ParamShoulderR',
  'ParamBodyUpper',
  'ParamHairAhoge',
  'ParamLegKnee',
  'ParamLeftLeg',
  'ParamRightLeg',
] as const

function report(values: Record<string, number>, rest: Record<string, number>): string {
  const moved = Object.entries(values)
    .filter(([id, v]) => Number.isFinite(v) && Math.abs(v - (rest[id] ?? 0)) > 0.05)
    .map(([id, v]) => `${id}=${v.toFixed(2)}`)
  return moved.length > 0 ? moved.join(' ') : '*** NADA ***'
}

const SWEEP: Array<{ label: string; overrides: Record<string, number> }> = [
  { label: 'controle-AngleZ-30', overrides: { ParamAngleZ: 30 } },
  { label: 'RA-menos1', overrides: { ParamArmRA: -1 } },
  { label: 'RA-zero', overrides: { ParamArmRA: 0 } },
  { label: 'RB-5', overrides: { ParamArmRB: 5 } },
  { label: 'RA-zero-RB-5', overrides: { ParamArmRA: 0, ParamArmRB: 5 } },
  { label: 'RA-menos1-RB-5', overrides: { ParamArmRA: -1, ParamArmRB: 5 } },
  { label: 'LA-zero-LB-5', overrides: { ParamArmLA: 0, ParamArmLB: 5 } },
  { label: 'ambos-B', overrides: { ParamArmRA: 0, ParamArmRB: 5, ParamArmLA: 0, ParamArmLB: 5 } },
]

test.describe('@probe canais de braço', () => {
  test.skip(!process.env.MIKA_LIVE2D_PROBE, 'sonda de homologação — rode com MIKA_LIVE2D_PROBE=1')

  test(`fotografa os canais de corpo da skin ${skin}`, async ({ app }, testInfo) => {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    await expect.poll(async () => listActions(page), { timeout: 30_000 }).toBe(true)

    const container = page.locator('.live2d-container')
    let restValues: Record<string, number> = {}

    for (const [label, action] of [
      ['vivo-repouso', null],
      ['vivo-wave', 'wave'],
      ['vivo-shrug', 'shrug'],
      ['vivo-cheer', 'cheer'],
    ] as const) {
      await page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.resume())

      await page.waitForTimeout(3000)
      if (action) await page.evaluate((name) => (window as BridgeWindow).__mikaLive2D?.playAction(name), action)
      await page.waitForTimeout(action ? 900 : 100)
      const values = await page.evaluate(
        (ids) => (window as BridgeWindow).__mikaLive2D?.readParams([...ids]) ?? {},
        BODY_CHANNELS,
      )
      if (label === 'vivo-repouso') {
        restValues = values
        console.log(`[${skin}] repouso`, JSON.stringify(values))
      } else {
        console.log(`[${skin}] ${label} moveu:`, report(values, restValues))
      }
      await container.screenshot({ path: testInfo.outputPath(`probe-${label}.png`) })
    }

    for (const step of skin === 'haru' ? SWEEP : []) {
      await page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.resume())
      await page.evaluate((overrides) => (window as BridgeWindow).__mikaLive2D?.probe(overrides), step.overrides)
      await page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.freeze(null, 0.4))
      await container.screenshot({ path: testInfo.outputPath(`probe-arm-${step.label}.png`) })
    }

    await page.evaluate(() => (window as BridgeWindow).__mikaLive2D?.resume())
  })
})
