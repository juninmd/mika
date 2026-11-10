import { join } from 'node:path'
import { VRM_BODY_ACTIONS } from '@mika/sdk/contracts'
import { ARM_REST, type ArmPose } from '../../src/renderer/src/components/avatar/vrm-body-rig'
import { VRM_ACTION_CLIPS } from '../../src/renderer/src/components/avatar/vrm-clips'
import { expect, test } from '../fixtures/tauri-app'

test.use({ skin: 'vrm-haru' })

// Gestos que trazem a mão para perto do corpo: o movimento mora mais no avanço do ombro
// (`rightUpperArmX`) e no cotovelo do que na abdução.
const CLOSE_TO_BODY = new Set(['shy', 'thinking', 'laugh'])

// `relaxed` e uma pose de DESCANSO: a tabela em vrm-body-poses.ts a define a ~0.13 do repouso, e o
// pico medido bate (0.130 em t=1.5s, de volta ao repouso em 3.75s). Cobrar dela os mesmos 0.15 das
// coreografias e cobrar um movimento que ela nunca teve — 0.08 ainda separa pose aplicada (0.13)
// de pose nao aplicada (ruido ocioso ~0.02).
const NEAR_REST = new Map([['relaxed', 0.08]])

const poseDelta = (pose: ArmPose, reference: ArmPose) =>
  Math.max(...Object.entries(reference).map(([key, value]) => Math.abs(pose[key as keyof ArmPose] - value)))

test('Haru VRM entra sozinho em uma pose ociosa visível', async ({ app }) => {
  const page = await app.firstWindow()
  await expect.poll(() => page.evaluate(() => window.__mikaVrm?.readLife().geometries ?? 0), { timeout: 30_000 }).toBeGreaterThan(0)
  const rest = await page.evaluate(() => window.__mikaVrm!.readArmRotations())

  await expect
    .poll(
      async () => {
        const pose = await page.evaluate(() => window.__mikaVrm!.readArmRotations())
        return Math.max(...Object.entries(rest).map(([key, value]) => Math.abs(pose[key as keyof typeof pose] - value)))
      },
      { intervals: [100], message: 'idle autônomo não saiu do repouso', timeout: 22_000 },
    )
    .toBeGreaterThan(0.08)

  await page.waitForTimeout(350)
  await page.screenshot({ path: join(__dirname, '../screenshots/haru-vrm-autonomous-idle.png') })
})

test('Haru VRM carrega a roupa e executa os gestos com retorno ao repouso', async ({ app }) => {
  test.setTimeout(360_000)
  const page = await app.firstWindow()
  await expect.poll(() => page.evaluate(() => Boolean(window.__mikaVrm)), { timeout: 30_000 }).toBe(true)
  const canvas = page.locator('canvas.live2d-container')
  await expect(canvas).toBeVisible()
  // Idle scheduler picks a random pose (up to relaxed's ~0.13 delta) every 5-14s — freeze it so
  // "voltou ao repouso" below measures the gesture, not a race against autonomous idle.
  await page.evaluate(() => window.__mikaVrm!.setIdleEnabled(false))
  await expect
    .poll(async () => poseDelta(await page.evaluate(() => window.__mikaVrm!.readArmRotations()), ARM_REST))
    .toBeLessThan(0.05)
  await page.screenshot({ path: join(__dirname, '../screenshots/haru-vrm-idle.png') })
  const rest = await page.evaluate(() => window.__mikaVrm!.readArmRotations())
  // Todas as coreografias do catálogo, não só as antigas: `presenting` e `heart`, por exemplo,
  // saem pela abdução esquerda, então o delta tem de olhar os dois braços.
  for (const action of VRM_BODY_ACTIONS) {
    await page.evaluate(name => window.__mikaVrm!.playAction(name), action)
    await expect.poll(async () => {
      const pose = await page.evaluate(() => window.__mikaVrm!.readArmRotations())
      return Math.max(
        Math.abs(pose.rightUpperArm - rest.rightUpperArm),
        Math.abs(pose.leftUpperArm - rest.leftUpperArm),
        Math.abs(pose.rightUpperArmX - rest.rightUpperArmX),
        Math.abs(pose.leftUpperArmX - rest.leftUpperArmX),
        Math.abs(pose.rightLowerArm - rest.rightLowerArm),
        Math.abs(pose.leftLowerArm - rest.leftLowerArm),
      )
    }, { message: `acao "${action}" nao moveu nenhum dos dois bracos` }).toBeGreaterThan(
      NEAR_REST.get(action) ?? 0.15,
    )
    // Ação com clip capturado (`VRM_ACTION_CLIPS`) decompõe o braço em outros eixos: só a pose procedural
    // tem o ombro em `rightUpperArmX` < -0.75; para o clip, o deslocamento acima já prova que moveu.
    if (CLOSE_TO_BODY.has(action) && !(action in VRM_ACTION_CLIPS)) {
      await expect.poll(async () => {
        const pose = await page.evaluate(() => window.__mikaVrm!.readArmRotations())
        return pose.rightUpperArmX
      }, { intervals: [50], message: `acao "${action}" nao trouxe a mao para perto do corpo` }).toBeLessThan(-0.75)
    } else {
      await page.waitForTimeout(350)
    }
    await page.screenshot({ path: join(__dirname, `../screenshots/haru-vrm-${action}.png`) })
    // `rest` e um retrato unico, mas o rig tem vida ociosa continua (respiracao, sway): igualdade
    // exata de float contra ele e inalcancavel por construcao. O que o teste quer provar e que a
    // coreografia TERMINOU — tolerancia bem abaixo do 0.15 que marca "se moveu" ainda pega gesto preso.
    await expect
      .poll(
        async () => {
          const pose = await page.evaluate(() => window.__mikaVrm!.readArmRotations())
          return poseDelta(pose, rest)
        },
        // Clips capturados chegam a ~12 s (`peaceSign` 11.7, `shoot` 9.6, `modelPose` 7.5) + fade de saída.
        { message: `acao "${action}" nao voltou ao repouso`, timeout: 20_000 },
      )
      .toBeLessThan(0.05)
  }
  for (const action of ['wink', 'shakehead']) {
    await page.evaluate(name => window.__mikaVrm!.playAction(name), action)
    await page.waitForTimeout(700)
    await page.screenshot({ path: join(__dirname, `../screenshots/haru-vrm-${action}.png`) })
    await page.waitForTimeout(1000)
  }
})
