# Prontidão para Steam — estado em 2026-10-06 (após revisão 360)

Verificado nesta branch: `pnpm lint`, `pnpm typecheck`, `pnpm build`, todos os `check:*` (inclui homologação fresca), `cargo test --lib` (3127), `cargo check --features steam` e `--features e2e`, vitest com cobertura (1492; thresholds mantidos).
**Não executado (não roda aqui):** E2E (Windows + `pnpm --filter @mika/desktop build:e2e`), `tauri build`, CI real, steamcmd, Steamworks real, hook NSIS e `bundleVCRuntime` num runner Windows.

## Implementado

- **Steam/auto-update:** build `--features steam` sem updater; CI (`release.yml` jobs `steam`/`steam-upload`, `tag`) publica no branch `beta` via steamcmd (`default` exige confirmação + `environment`); depot (`steam/assemble-depot.sh`, `lib/Mika` + `legal/`); Steamworks (init, Rich Presence, 5 conquistas). Build direta: updater assinado + `latest.json`, backup pré-update completo e atômico.
- **CI/supply chain:** `ci.yml` (PR), `audit.yml` semanal (cargo-deny + advisories, pnpm audit, gitleaks), Dependabot, chave de assinatura só no passo de empacotar, `Cargo.lock` único + `--locked`.
- **Segurança:** SSRF (resolver que filtra IP privado + tabela única), aprovação obrigatória para tools de risco chamadas pelo LLM (`ToolOrigin`), regra anti-exfiltração por URL, MCP com `env_clear` + env cifrado, allowlist no `desktop launch`, denylist de combos de teclado, `on_navigation`, hooks E2E atrás da feature `e2e`, sandbox JS que mata a thread.
- **Conteúdo/privacidade:** piso de segurança no prompt + detector de crise determinístico (10 idiomas), gate de idade 18+ (Companion exige), botão de reportar resposta, aviso de IA, rótulos de conteúdo não confiável, ZDR no OpenRouter, defaults opt-in (busca web, computer-use, catálogo de pessoas), consentimento de EULA/PRIVACY no wizard + modal, wipe/export completos, redação de chaves nos logs, pausa de mic/tela na bandeja, blocklist de captura ampliada.
- **Robustez:** resumo do chat apagado no clear, registro de turnos por token, abort em tool, panic isolado, timeouts HTTP, Whisper >30 s, CJK (frases/esquecer/busca), scrypt com cache, config atômico + `.bak`, logs com rotação, migrações SQLite versionadas, charset de páginas.
- **UI/i18n:** 10 idiomas, erros de `invoke` visíveis, mic/WebGL/VRM sem vazamento, acessibilidade (foco no wizard, tabs, select), formatação por locale, Supertonic→Piper (vozes CC0) e vozes Azure/Google por idioma.

## Pendente — exige decisão ou ação humana

1. **Jurídico:** preencher placeholders do EULA/PRIVACY (`[RAZÃO SOCIAL]`, `[E-MAIL DE SUPORTE]`, endereço, foro, DPO, vigência), revisão jurídica, versão em inglês (os documentos são só pt-BR); linhagem das vozes Piper (finetunes do checkpoint Lessac) e termos do Supertonic/Nemotron/`.vrma` VRoid/Live2D Cubism Core.
2. **Secrets e infra:** `STEAM_*`, `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)`, var `TAURI_UPDATER_PUBKEY`, environments `release`/`steam-production` com revisores; Authenticode/Apple opcionais.
3. **Steamworks:** criar app/depots/conquistas (nomes em `steam/README.md`), questionário de IA (citar só os guardrails reais), IARC; **não habilitar Auto-Cloud** para dados privados.
4. **Rodar de verdade:** E2E (rebuild com feature `e2e`), primeiro build `beta`, depot Linux (glibc/webkit no runtime Steam), Steam Deck/Wayland, instalador Windows (hook NSIS, VC++/DirectML).
5. **Produto:** conferir telefones do cartão de crise (Yorisoi, 109 KR, 12356 CN); idade é autodeclarada; `mood.habitsEnabled` segue ligado por padrão; trocar piso de segurança por prompt-overrides/customInstructions pode enfraquecê-lo em texto (o detector em código não).
6. **Adiado por escopo:** consentimento por provedor remoto no 1º envio; aviso ao esconder a janela com sentido ligado, hotkey de pânico e atalhos globais rebindáveis; senha no zip de exportação; ACL do `master.key` no Windows; embeddings em BLOB (300 ms/consulta com 1500 vetores); guard de "fiz X" para claims sem nenhuma tool fora do pt; `desktop.rs` ainda tem textos PT; `biome` não cobre `e2e/` (76 erros a limpar); `pnpm audit` com 2 vulnerabilidades transitivas (braces/katex) e crate `yoke-derive` yanked.

## 2ª revisão adversarial (2026-10-06) — corrigido

- Aprovação "esta sessão" só em memória; panic no sandbox não trava mais `code`; exfiltração por path/subdomínio/`web search` longo exige aprovação; MCP preserva `env` ilegível (`unreadableEnv`) e herda variáveis suficientes (sem proxies); denylist de combos Meta; `master.key` atômica; `on_navigation` com porta.
- Detector de crise sem falso negativo por "frase benigna" (scrub por token, NFKC/NFD) e sem falso positivo de jogo; `effect_guard` por ação; piso de segurança não removível por override (+ `check:prompt-files` real); nota de crise por último; `crisisRegion` também em erro; 404 de ZDR classificado.
- Config: migração que preserva `browser`/`computer-use` de perfis existentes, `.bak` só em erro de parse; logs sobrevivem ao "apagar meus dados"; blocklist de captura sem colisões com jogos (`proc:`); aviso Wayland; pausa de sentidos chega ao HUD.
- Gate legal com foco/`inert`, falha fechada; CRLF no hash de licenças (Windows); gates novos `check:release-docs`; CI com `--features steam`.
- Pendente desta rodada: estado da pausa de sentidos se perde ao recarregar a janela; HasRunKey do `installscript.vdf` não validado no steamcmd; registro "vous" majoritário no francês (só o grupo legal foi unificado).
