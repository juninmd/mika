import { join } from 'node:path'
import type { Page } from '@playwright/test'
import { ARM_REST, type ArmPose } from '../../src/renderer/src/components/avatar/vrm-body-rig'
import { VRM_LOOKS } from '../../src/renderer/src/components/avatar/vrm-look'
import { callE2eHook, expect, test } from '../fixtures/tauri-app'

const shot = (name: string) => join(__dirname, `../screenshots/vrm-life-${name}.png`)
const life = (page: Page) => page.evaluate(() => window.__mikaVrm!.readLife())
const poseDelta = (pose: ArmPose, reference: ArmPose) =>
  Math.max(...Object.entries(reference).map(([key, value]) => Math.abs(pose[key as keyof ArmPose] - value)))

async function waitForVrm(page: Page): Promise<void> {
  await expect.poll(() => page.evaluate(() => Boolean(window.__mikaVrm)), { timeout: 30_000 }).toBe(true)
  await expect(page.locator('canvas.live2d-container')).toBeVisible()
  await expect.poll(() => page.evaluate(() => window.__mikaVrm?.readLife().geometries ?? 0), { timeout: 30_000 }).toBeGreaterThan(0)
}

async function sample(page: Page, ms: number, pick: (s: Awaited<ReturnType<typeof life>>) => number) {
  const values: number[] = []
  for (const end = Date.now() + ms; Date.now() < end; ) {
    values.push(pick(await life(page)))
    await page.waitForTimeout(40)
  }
  return values
}

// `readLife().textures/geometries` count VRM material/spring/accessory GPU resources that finish
// arriving over the first few seconds after `waitForVrm` (which only requires >0, not "done").
// Sampling `before` mid-arrival made a legitimate late allocation look like a resize-triggered leak
// (diagnosed by sampling every resize: counts jumped once, at whatever resize happened to land after
// the pipeline settled, then stayed exactly flat for 6+ further resizes — not a per-resize growth).
// Waiting for 3 consecutive stable reads gives resize a true baseline instead of a moving target.
async function waitForGpuResourcesToSettle(page: Page): Promise<void> {
  let stableReads = 0
  let last = { textures: -1, geometries: -1 }
  const deadline = Date.now() + 25_000
  while (stableReads < 3 && Date.now() < deadline) {
    const { textures, geometries } = await life(page)
    if (textures === last.textures && geometries === last.geometries) stableReads++
    else stableReads = 1
    last = { textures, geometries }
    await page.waitForTimeout(300)
  }
}

test.describe('vida do VRM no app real', () => {
  test.use({ skin: 'vrm-haru' })

  // Cursor arrived at the avatar in 0..1 but both avatars expect -1..1, so half the screen was never looked at.
  test('a cabeça segue o cursor para os dois lados', async ({ app }) => {
    const page = await app.firstWindow()
    await waitForVrm(page)
    const { width, height } = page.viewportSize() ?? (await page.evaluate(() => ({ width: innerWidth, height: innerHeight })))
    for (const [x, sign] of [[4, -1], [width - 4, 1]] as const) {
      await expect
        .poll(async () => {
          await page.mouse.move(x, height * 0.3)
          return (await life(page)).headYaw * sign
        }, { timeout: 5_000 })
        .toBeGreaterThan(0.05)
      await page.screenshot({ path: shot(sign < 0 ? 'look-left' : 'look-right') })
    }
    const { neckYaw, headYaw } = await life(page)
    expect(Math.sign(neckYaw)).toBe(Math.sign(headYaw))
  })

  test('pisca em repouso e não pisca por cima do sorriso', async ({ app }) => {
    const page = await app.firstWindow()
    await waitForVrm(page)
    const idle = await sample(page, 8_000, (s) => s.blink)
    expect(Math.max(...idle), 'nenhuma piscada em 8 s').toBeGreaterThan(0.3)
    // Clicking the avatar switches its expression to "excited" (happy) for 2s.
    const box = await page.locator('canvas.live2d-container').boundingBox()
    await page.mouse.click(box!.x + box!.width / 2, box!.y + box!.height * 0.3)
    await expect.poll(async () => (await life(page)).happy).toBeGreaterThan(0.5)
    const whileHappy = await sample(page, 1_200, (s) => (s.happy >= 0.5 ? s.blink : 0))
    expect(Math.max(...whileHappy)).toBe(0)
    await page.screenshot({ path: shot('happy') })
  })

  test('molas se movem com a brisa e continuam finitas', async ({ app }) => {
    const page = await app.firstWindow()
    await waitForVrm(page)
    const poses = await sample(page, 6_000, (s) => s.springPose)
    expect(Math.max(...poses) - Math.min(...poses), 'saia, cacho e lenço parados').toBeGreaterThan(0.002)
    expect((await life(page)).springFinite).toBe(true)
  })

  // TTS real fica desligado no E2E (custo/determinismo); driveLipSync escreve nos mesmos refs que o
  // analisador de áudio escreveria, provando o caminho volume/vogal -> viseme -> blend shape no app
  // de verdade — o que nenhum teste de unidade, isolado do render, consegue provar.
  test('a boca sincroniza com volume e vogal sintéticos, e fecha quando o áudio para', async ({ app }) => {
    const page = await app.firstWindow()
    await waitForVrm(page)
    expect((await life(page)).mouth, 'boca já aberta em repouso').toBeLessThan(0.05)

    // > IDLE_OPEN_CAP (0.15, vrm-mouth.ts): sem `speaking=true` a boca nunca passaria dali, provando
    // só o ruído ocioso e não o range real de fala.
    await page.evaluate(() => window.__mikaVrm!.driveLipSync(1, { a: 1, i: 0, u: 0, e: 0, o: 0 }))
    await expect
      .poll(async () => (await life(page)).mouth, { message: 'boca não abriu no range de fala real' })
      .toBeGreaterThan(0.5)
    await page.screenshot({ path: shot('lipsync-open') })

    await page.evaluate(() => window.__mikaVrm!.driveLipSync(0, { a: 0, i: 0, u: 0, e: 0, o: 0 }))
    await expect
      .poll(async () => (await life(page)).mouth, { message: 'boca não fechou ao silenciar' })
      .toBeLessThan(0.05)
  })

  // `VrmCanvas` mantém o acabamento anime (contorno, tom dividido, saturação) ligado em qualquer
  // estado de energia ou foco: é a identidade de sombreamento do modelo, e desligá-lo na bateria
  // deixava o avatar chapado quase o tempo todo (a janela do companheiro vive sem foco, sobre o jogo).
  // Só a taxa de quadros escala. O sinal vem do gancho `power-signal` (`src-tauri/.../e2e_hooks.rs`),
  // que publica `EventKey::PowerBattery`/`PowerAc` no mesmo barramento do `system_signals/power.rs`.
  test('acabamento anime continua ligado na bateria e ao voltar para a tomada', async ({ app }) => {
    const page = await app.firstWindow()
    await waitForVrm(page)
    for (const onBattery of [true, false]) {
      await callE2eHook(page, 'power-signal', { kind: onBattery ? 'battery' : 'ac' })
      await page.waitForTimeout(1200)
      expect((await life(page)).postFx, onBattery ? 'na bateria' : 'na tomada').toBe(true)
      await page.screenshot({ path: shot(`postfx-${onBattery ? 'battery' : 'ac'}`) })
    }
  })

  test('sobrevive a redimensionar sem vazar GPU', async ({ app }) => {
    const page = await app.firstWindow()
    await waitForVrm(page)
    await waitForGpuResourcesToSettle(page)
    const before = await life(page)
    for (const scale of [0.8, 1.2, 1]) {
      await callE2eHook(page, 'window-set-size', { width: 400 * scale, height: 640 * scale })
      await expect.poll(async () => (await life(page)).canvasWidth).not.toBe(0)
    }
    await expect.poll(async () => (await life(page)).canvasWidth).toBe(before.canvasWidth)
    const after = await life(page)
    expect(after.textures).toBeLessThanOrEqual(before.textures)
    expect(after.geometries).toBeLessThanOrEqual(before.geometries)
    await page.screenshot({ path: shot('after-resize') })
  })
})

// Every VRM skin goes through material finishing; "flat" ones drop the base texture.
for (const look of VRM_LOOKS) {
  test.describe(`skin VRM ${look.id}`, () => {
    test.use({ skin: `vrm-${look.id}` })

    test('carrega com acabamento anime e entra em movimento ocioso, sem erro no console', async ({ app }) => {
      const page = await app.firstWindow()
      const errors: string[] = []
      page.on('console', (msg) => {
        if (msg.type() === 'error' && /vrm|three|webgl|shader/i.test(msg.text())) errors.push(msg.text())
      })
      await expect.poll(() => page.evaluate(() => Boolean(window.__mikaVrm)), { timeout: 30_000 }).toBe(true)
      await page.evaluate(() => window.__mikaVrm!.setIdleEnabled(false))
      await waitForVrm(page)
      await page.evaluate(() => window.__mikaVrm!.setIdleEnabled(false))
      await expect
        .poll(async () => poseDelta(await page.evaluate(() => window.__mikaVrm!.readArmRotations()), ARM_REST))
        .toBeLessThan(0.05)
      const rest = await page.evaluate(() => window.__mikaVrm!.readArmRotations())
      await page.evaluate(() => window.__mikaVrm!.setIdleEnabled(true))
      await expect
        .poll(
          async () => {
            const pose = await page.evaluate(() => window.__mikaVrm!.readArmRotations())
            return poseDelta(pose, rest)
          },
          { intervals: [100], message: `skin ${look.id} não saiu do repouso`, timeout: 22_000 },
        )
        .toBeGreaterThan(0.08)
      await page.waitForTimeout(350)
      const state = await life(page)
      expect(state.springFinite).toBe(true)
      expect(state.textures).toBeGreaterThan(0)
      await page.screenshot({ path: shot(`skin-${look.id}`) })
      expect(errors).toEqual([])
    })
  })
}
