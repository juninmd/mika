# E2E (Playwright)

Dirige o **binário release do Tauri** (WebView2) por CDP. Regras de AGENTS.md §1/§5 valem aqui; este
arquivo registra só o que é específico da suíte.

## Windows, Linux e macOS

Windows (WebView2) expõe CDP: o fixture (`fixtures/tauri-app.ts`) sobe `mika-desktop.exe` com
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=…` e conecta o Playwright por
`connectOverCDP`. WebKitGTK (Linux) e WKWebView (macOS) **não** expõem CDP, então ali o fixture usa o
**host HTTP do E2E** (`fixtures/e2e-host.ts` ↔ `src-tauri/src/e2e_host.rs`, só na feature `e2e`):

- o binário serve o bundle embarcado e o IPC em `127.0.0.1:<porta>` (`MIKA_E2E_HOST_PORT`, com
  `MIKA_E2E_HOST_TOKEN` obrigatório em toda rota `/__e2e/*`);
- o Playwright abre um **Chromium** por janela que o backend criou e injeta um shim de
  `window.__TAURI_INTERNALS__` que leva `invoke`/`listen` — as únicas duas funções do Tauri que o
  renderer usa — ao backend Rust **real** (`bridge::dispatch`, o mesmo corpo de `mika_invoke`);
- as janelas nativas continuam existindo, mas em `about:blank`: só a página do Chromium roda a UI.

As specs são as mesmas nos três sistemas (`app.firstWindow()` devolve uma `Page` real). `MIKA_E2E_HOST=1`
força o host também no Windows. Em Linux precisa de display e de um barramento D-Bus de sessão:
`xvfb-run -a dbus-run-session -- pnpm test`. Limites do host: o esquema `mika-live2d:` (amostras
Live2D) não existe no Chromium, então specs de skin Live2D rodam só no caminho CDP.

### Estado da suíte no host (medido em Linux/xvfb)

Rodada completa no host: **88 de 109 specs passam, 1 pulado**. O que sobra não é do host — a CI
(`ci.yml`, job `e2e`) roda só `@smoke` até isto ser resolvido:

| Spec | Causa |
|---|---|
| `homologation-capture-all` (04), `homologation-extras` (3) | esperam o wizard de **4** etapas; ele tem 8 (`WIZARD_STEPS`) |
| `memory-reaction` (F3, F3-boundaries, F4, F5-C-TC1/TC4, F6-W-TC2/TC4/TC5) | `real_person` não entra mais sozinho no catálogo (`vision::watcher::may_auto_catalog`); os specs ainda emitem `real_person`/`medium` |
| `user-journey-all-features` | espera resposta do assistente por 60 s no ambiente lento do contêiner |
| `controls-audit` | `keepawake` pede o barramento de sistema do D-Bus (ausente em contêiner); orçamento de 900 s |
| `haru-vrm`, `vrm-life` (3) | animação em tempo real: com GL por software (`swiftshader`) o quadro não anda no ritmo esperado |
| `settings-navigation-full` | passa em ~3 min num contêiner de 4 núcleos; o `setTimeout` do spec é 60 s |

## Como rodar

```bash
pnpm --filter @mika/desktop build:e2e   # tauri build --no-bundle --features e2e, sem LTO; NUNCA `cargo build --release` cru
pnpm test                                                  # ou: playwright test -c e2e/playwright.config.ts e2e/specs/<spec>.spec.ts
```

A feature `e2e` compila os ganchos de teste (`mika:e2e-hook`, override da Wikipédia): o binário
distribuído não os tem, e este não deve ser distribuído. O binário é procurado em `<raiz>/target/e2e/release/mika-desktop[.exe]` (alvo próprio, sem LTO: `scripts/dev/build-e2e.mts`; o release de verdade levava ~15 min por mudança), ou em
`$CARGO_TARGET_DIR/release/` se a variável estiver definida — resolvido em um só lugar,
`apps/desktop/e2e/fixtures/app-binary.ts`.

## Artefatos

Todo spec termina com um artefato verificável em `apps/desktop/e2e/screenshots/` (recriada a cada run por
`global-setup.ts`): PNG (`page.screenshot`, `artifactPath()`) e/ou JSON com o estado que o spec
afirmou (`writeArtifact()` em `apps/desktop/e2e/fixtures/artifact.ts`). Abra os PNGs: gate verde não prova tela.

## Esperas

Sem `waitForTimeout` para "deixar assentar": use `expect.poll`/`toBeVisible`, ou `waitForUiIdle`
(`apps/desktop/e2e/fixtures/settled.ts`) antes de um print. Os poucos tempos que restam (`haru-vrm`, `vrm-life`,
`live2d-arm-probe`) são relógio de coreografia do avatar, sem API que diga "o gesto chegou ao pico".

## Perfil e `legal`

Os fixtures gravam `config.json` com `legal: legalAccepted()` (`apps/desktop/e2e/fixtures/legal-accepted.ts`): sem o
aceite da versão atual de EULA/PRIVACY o `LegalGate` cobre a UI inteira. `MIKA_HOMOLOG_FRESH=1`
(homologação) pula esse seed de propósito — é o primeiro boot real, com o diálogo.
