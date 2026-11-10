import { join } from 'node:path'
import { clickDockButton } from '../fixtures/dock'
import { waitForUiIdle } from '../fixtures/settled'
import { capturePagePixels, expect, test } from '../fixtures/tauri-app'

// Relative to the spec, not cwd — see voice-waveform.spec.ts.
const SCREENSHOTS_DIR = join(__dirname, '..', 'screenshots')

type ElectronApi = { electronAPI: import('../../src/shared/electron-api-types').ElectronShellApi }

// FIXED PRODUCT BUG, uncovered while fixing `capture-main-window` (see `edgeAlphas` below) and
// root-caused in `src-tauri/src/modules/settings/mod.rs`: `CH_SET` (`mika:settings-set`) never
// emitted `config-changed` AT ALL — not "only on a second call". `saveSetting` always resolved
// `{ accepted: true }` and the store always held the new value, but no listener anywhere (fresh or
// old) ever received the event, because the emit code was entirely absent (confirmed against
// `git show HEAD:...settings/mod.rs` — no `EVENT_CONFIG_CHANGED`, no `emit_to`, at the commit this
// binary was built from). `useDesktopShellBootstrap`/`useSettingsController` only update local
// state from this event — `saveSetting`'s own `.then()` never writes back — so every Settings
// control was silently stuck on the old value until a manual reload
// (`homologation-remaining-surfaces.spec.ts`'s Telemetry Opt-In test's `page.reload()` is for an
// UNRELATED reason — `setTelemetryOptIn` is its own `mika:telemetry-opt-in` channel, not
// `mika:settings-set`, and re-triggers boot-time gating on purpose — so it stays as is). Fixed by
// adding `EVENT_CONFIG_CHANGED`/`broadcast_config_changed` to `CH_SET`, emitted on EVERY successful
// set; regression-tested in `modules::settings::tests::two_consecutive_sets_emit_config_changed_twice`
// (`cargo test --lib settings::`). Needs a binary rebuild to verify end-to-end here — Rust source
// changed. The assertion below was never weakened.
async function setBackgroundMode(page: import('@playwright/test').Page, mode: string) {
  await page.evaluate(async (m) => {
    const api = (window as unknown as ElectronApi).electronAPI
    await api.runtime.saveSetting('character.backgroundMode', m)
  }, mode)
  // The fill fades over 1 s; a fixed wait photographed mid-transition (alpha ~0.67 and a cut at the dock).
  const fill = page.getByTestId('stage-backdrop-fill')
  if (mode === 'transparent') {
    await expect(fill).toHaveCount(0)
    return
  }
  const target = mode === 'solid' ? '1' : '0.35'
  await expect.poll(() => fill.evaluate((el) => getComputedStyle(el).opacity)).toBe(target)
}

// Ported off Electron's `webContents.capturePage()` (main-process only, no Tauri equivalent) — first
// tried the `capture-main-window` e2e hook (`src-tauri/src/modules/e2e_hooks.rs`, `xcap`/GDI), but
// that path can NEVER see this window: `xcap::Window::all()` excludes windows owned by its OWN
// process by design (WebRTC-derived self-capture guard, `windows/impl_window.rs` in the crate), so
// "the GDI/DWM alpha caveat" documented there was masking a harder failure underneath (`all()`
// returns zero windows for this process, full stop — not a partial/alpha-only gap). `capturePagePixels`
// (`fixtures/tauri-app.ts`) instead reads the REAL composited page via `page.screenshot({
// omitBackground: true })` — the same Chromium/WebView2 compositor that paints the screen, which DOES
// carry real alpha for a page with no opaque background. Multiplatform for free: no OS capture API,
// no `cfg` branch.
async function edgeAlphas(page: import('@playwright/test').Page): Promise<number[]> {
  const probe = await capturePagePixels(page, [[2, 0]])
  const points = [2, Math.floor(probe.height / 2), probe.height - 1].map((y) => [2, y] as [number, number])
  const captured = await capturePagePixels(page, points)
  return captured.samples.map((sample) => sample[3])
}

const MODES = ['transparent', 'semi-transparent', 'solid'] as const
const SUFFIX = process.env.MIKA_E2E_BG_SUFFIX ?? 'after'

// Visual proof the avatar stays in focus across backgroundMode; only exists for its screenshots to be read.
test('avatar mantém o foco visual nos três backgroundMode, em repouso e falando', async ({ app, llm }) => {
  // O dublê responde em ~5 ms: sem atraso o "pensando" acende e apaga entre dois quadros e a asserção
  // de que ele aparece corre contra o relógio. 0,8 s é folgado para o polling de 100 ms do Playwright amostrar.
  llm.setChatDelay(800)
  // Três modos × um turno de chat cada + a janela de observação pós-turno: passa de 120 s numa partida a frio.
  test.setTimeout(240_000)
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await expect(page.getByTestId('voice-waveform')).toBeVisible({ timeout: 20_000 })
  // Let boot activity (proactive greeting, initial load) settle first, or the first mode inherits boot's turn.
  await expect(page.getByText(/PENSANDO/i)).toHaveCount(0, { timeout: 15_000 })

  for (const mode of MODES) {
    await setBackgroundMode(page, mode)
    // The compositor hands back the previous frame right after a switch; the property is the settled frame.
    if (mode === 'solid') await expect.poll(() => edgeAlphas(page), { message: 'solid must cover the whole window' }).toEqual([255, 255, 255])
    if (mode === 'transparent')
      await expect.poll(async () => Math.min(...(await edgeAlphas(page))), { message: 'transparent must stay see-through' }).toBeLessThan(255)

    // Move off any button so a tooltip can't stay lit just because the test's synthetic cursor never left.
    await page.mouse.move(4, 4)

    await expect(page.getByText(/PENSANDO/i)).toHaveCount(0)
    await expect(page.getByRole('tooltip')).toHaveCount(0)
    await page.screenshot({ path: join(SCREENSHOTS_DIR, `avatar-bg-${mode}-idle-${SUFFIX}.png`) })

    await clickDockButton(page, 'Chat (Ctrl+Space)')
    const input = page.locator('textarea[name="prompt"]')
    await expect(input).toBeVisible()
    await page.mouse.move(4, 4)
    await page.screenshot({ path: join(SCREENSHOTS_DIR, `avatar-bg-${mode}-composer-${SUFFIX}.png`) })

    // Real chat turn via fake LLM — covers thinking/speaking.
    await input.fill(`oi mika, me conta uma curiosidade sobre ${mode}`)
    await page.keyboard.press('Enter')
    // Duas peças legítimas mostram o estado (pílula do HUD e texto do indicador): basta uma visível.
    await expect(page.getByText(/PENSANDO/i).first()).toBeVisible()
    await waitForUiIdle(page)
    await page.screenshot({ path: join(SCREENSHOTS_DIR, `avatar-bg-${mode}-thinking-${SUFFIX}.png`) })

    // The fake LLM replies after `setChatDelay` (0.8s); the turn is over when the thinking indicator goes away. Waiting for that
    // (not a fixed sleep) is what keeps the next mode from inheriting this turn instead of starting from idle.
    await expect(page.getByText(/PENSANDO/i)).toHaveCount(0, { timeout: 15_000 })
    await waitForUiIdle(page)
    await page.screenshot({ path: join(SCREENSHOTS_DIR, `avatar-bg-${mode}-speaking-${SUFFIX}.png`) })

    await page.keyboard.press('Escape')
    await page.mouse.move(4, 4)
    await waitForUiIdle(page)

    // Regression guard: post-turn enrichment (fact/context/mood) is invisible and must not relight the HUD/bubble.
    // It emits no event, so observe a window (the old fixed sleep + one sample could miss a flash): the
    // thinking indicator must not show up at any frame while the enrichment cycle runs.
    const relit = await page
      .waitForFunction(() => /PENSANDO/i.test(document.body.innerText), undefined, { timeout: 3500 })
      .then(
        () => true,
        () => false,
      )
    expect(relit, 'o enriquecimento pós-turno reacendeu o indicador de pensamento').toBe(false)
    await expect(page.getByRole('tooltip')).toHaveCount(0)
  }
})
