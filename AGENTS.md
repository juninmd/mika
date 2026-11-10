# Mika — Guia do Agente

VTuber AI companion para games, código e cultura nerd. Roda local no PC do usuário.

---

## 1. Stack & Comandos Oficiais

- **Stack:** Tauri 2 (Rust, backend) · Node 24 (tooling/renderer build) · React 19 / Tailwind 4 · Playwright 1.64 · Vitest 5 · Biome 2 · TypeScript 7.
  Electron saiu por inteiro na migração Tauri (Stage C, 2026-09-22): `src/main` e o backend de
  `src/features/*` viraram `apps/desktop/src-tauri`; o que sobrou em TypeScript é o renderer (web
  bundle) e a ponte `src/renderer/bridge` (ex-preload, mesmo contrato, falando com `invoke`/`listen`
  do Tauri em vez de `ipcRenderer`).
- **Linter:** Estritamente Biome de código (`biome check .`).
- **Testes — duas camadas, cada uma com seu escopo:**
  - **E2E (Playwright, `apps/desktop/e2e`)** — a única prova válida para UI e para o app montando de
    verdade. Mudou tela, tem de passar por aqui, e os PNGs em `apps/desktop/e2e/screenshots/` têm de
    ser abertos: gate verde não prova tela.
  - **Unidade e contrato (Vitest)** — lógica pura: recorte de frase no stream, wake-word, gate de
    aprovação, resolução de provedor, política ZDR. Subir um Electron inteiro para responder uma
    pergunta de microssegundos custa segundos, e o que custa segundos não é rodado.
  > **Por quê:** esta linha já disse "exclusivamente E2E", enquanto `specs/11-quality-gates.md`
  > exigia `pnpm test:contracts` e afirmava que gate cuja pergunta depende de OBJETO em runtime é
  > teste de contrato, não script. As duas instruções checadas no repositório se contradiziam. Na
  > primeira vez que a camada de unidade existiu, ela achou três defeitos reais no mesmo dia — um
  > handler de aprovação que lançava síncrono e escapava para o loop de ferramentas, um `idle` que
  > nunca era aplicado, e o estado `speaking` que não emitia nada.
- **Gates:** `pnpm gates` roda a bateria de invariantes (`scripts/checks/`) e, no fim, `test:rust`.
  Cada gate existe porque a classe de defeito que ele pega já aconteceu aqui; a história está no
  cabeçalho de cada script.
  > Os testes Rust entraram na bateria em 2026-09-21: três deles (`bridge.rs`, `permissions.rs`,
  > `shortcuts.rs`) LEEM o fonte TypeScript para provar que os dois lados não divergiram, e não
  > rodavam em passo nenhum — gate que não roda não é gate, é comentário que compila.
- **Execução Econômica:** Sempre rode comandos em modo `--quiet` / `--silent` / silencioso para poupar tokens e saídas verbosas desnecessárias.

| Comando | O que faz |
|---|---|
| `pnpm lint` | Validação de código com Biome (`biome check .`) |
| `pnpm gates` | Bateria de gates de invariante (`scripts/checks/`) + `test:rust` |
| `pnpm test:rust` | Testes do shell Tauri (`cargo test --lib`), inclusive os de contrato cross-language |
| `pnpm typecheck` | Typecheck estrito em todos os workspaces |
| `pnpm test:unit` | Unidade e contrato (Vitest) com catraca de cobertura (`thresholds` em `vitest.config.mts`: só sobe) |
| `pnpm build` | Build do renderer + prompts (`build:prompts && build:renderer`, via `vite.tauri.config.ts`) |
| `pnpm --filter @mika/desktop tauri:build` | Bundle nativo completo (instalador) via Tauri CLI |
| `pnpm test` | Executa a suíte E2E via Playwright (`apps/desktop/e2e`) |
| `pnpm test:e2e:headed` | Roda testes E2E com janela visível |
| `pnpm dev` | Inicia o app em modo desenvolvimento (`tauri dev`) |
| `pnpm live2d:gen` | Regenera motions do avatar (`scripts/live2d/live2d-gen-motions.ts`) |
| `pnpm live2d:thumbs` | Regenera miniaturas das skins |

---

## 2. Invariantes de Arquitetura (v5)

1. **Topologia Modular:**
   - **Backend (Rust, `apps/desktop/src-tauri/src/modules/<ctx>/`):** núcleo bloqueante (`brain`,
     `browser`, `chat`, `inference`, `llm`, `memory`, `telemetry`, `voice`, `vision`), contextos
     próprios que não são feature tardia (`modules/gamer/` — gamer-companion —, `modules/mcp/`) e
     features tardias (`modules/features/<ctx>/`: `character`, `code_runner`, `computer_use`,
     `desktop`, `mood`, `person_catalog`, `privacy`, `scheduler`, `user`). Roster: `mod <feature>;`/
     `pub mod <feature>;` em `modules/features/mod.rs` — pasta sem registro é erro de compilação, não gate.
   - **UI (`apps/desktop/src/features/<ctx>/renderer/`):** cada feature é dona da sua própria UI
     (`apps/desktop/src/features/<ctx>/renderer/settings.tsx`), registrada em
     `apps/desktop/src/renderer/src/lib/feature-ui-registry.ts`. É tudo que resta de
     `src/features/<ctx>/` em TypeScript — o backend de cada uma virou Rust (Stage C, 2026-09-22).
   - Backend Rust nunca lê TypeScript em runtime (só `include_str!`/`include_bytes!` de fonte de
     dado compartilhada, ex. `character.schema.ts` para a tab de personagem — ver `check:rust-include-paths`).

2. **Layout e nomenclatura de módulo** — a mesma pergunta tem de ter a mesma resposta em todo módulo.
   Antes desta padronização, "onde mora a implementação de uma tool?" tinha três respostas diferentes
   (`handlers/`, `tools/`, `tools.ts` solto), e "qual o sufixo de um serviço?" tinha duas
   (`-service.ts` e `.service.ts`). Vale para `packages/sdk`/`packages/sdk-ui` (só
   contratos, tipos, i18n e UI compartilhada — o legado Node saiu em 2026-09-23) e para o `.module.ts` que sobrevive em cada `apps/desktop/src/features/<ctx>/` só
   como manifesto de posse do `renderer/` (homologação/ownership) — `services/`, `tools/`,
   `handlers/`, `prompts/` de uma feature de `apps/desktop` agora moram em
   `apps/desktop/src-tauri/src/modules/features/<ctx>/`, com convenção própria de Rust (um
   `mod.rs` por feature, testes com `#[cfg(test)]` no mesmo arquivo).

   ```
   <módulo>/
     <nome>.module.ts     manifesto: contrato, deps, eventos, config, frozen
     <nome>.feature.ts    tools e settings declarados ao registry
     <nome>.schema.ts     schema de config (sufixo `-config.schema.ts` quando há mais de um)
     index.ts             raiz de composição — o que o app carrega
     ports.ts             ÚNICO ponto do módulo que toca o container
     services/            lógica: `<nome>.service.ts`, `<nome>.runtime.ts`
     tools/               onde a tool é DECLARADA (`= tool({...})`)
     handlers/            implementação por trás das tools
     prompts/             markdown
     renderer/            UI própria
     __tests__/           suíte de contrato
   ```

   - **Arquivo nunca se chama `index.<algo>.ts`.** `index` é reservado para o barril; um schema é
     `voice.schema.ts`, não `index.schema.ts`.
   - **Sufixo composto com ponto**, não hífen: `.service.ts`, `.runtime.ts`, `.schema.ts`,
     `.feature.ts`, `.module.ts`. `chat-service.ts` está errado, `chat.service.ts` está certo.
   - **PascalCase só para componente React**; todo o resto é kebab-case.
   - **`prompt-loader.ts` de cada módulo NÃO se move.** Ele resolve a pasta de prompts a partir do
     próprio diretório; mudá-lo de lugar faz o prompt sumir em silêncio, sem erro e sem log.

3. **Isolamento de Processos (Tauri):**
   - **Renderer:** Zero imports Node (`fs`, `path`, etc.). Consome o backend Rust exclusivamente via
     `window.electronAPI` — a ponte (`apps/desktop/src/renderer/bridge/`, ex-preload) mantém o mesmo
     nome global e contrato de sempre, só que fala `invoke`/`listen` do Tauri em vez de `ipcRenderer`.
   - **IPC Centralizado:** Todos os canais e rotas tipados em `packages/sdk/src/contracts/desktop-ipc.ts`.
   - **Guarda de Superfície:** Rotas sensíveis invocam `assertOwnerSurface` (Rust,
     `apps/desktop/src-tauri/src/modules/`) gateando pela janela de origem.
4. **EventBus & DI:**
   - Comunicação cross-module exclusivamente por eventos tipados: barramento Rust
     (`apps/desktop/src-tauri/src/events/`), chaves espelhadas em `packages/sdk/src/svc/events.ts`.
   - Sem container DI em TypeScript: o estado do backend é gerenciado pelo Tauri (`app.manage`).
5. **Áudio & Inferência:**
   - STT (Whisper) e TTS (Supertonic/Kokoro) rodam em Rust (`apps/desktop/src-tauri/src/modules/inference/`).
   - Backend aceita apenas PCM16 mono 16 kHz. Decodificação de áudio no Renderer.
6. **LLM & Prompts:**
   - LLM local padrão: Ollama. Provedores remotos resolvidos em Rust (`apps/desktop/src-tauri/src/modules/llm.rs`,
     `modules/brain/`); o catálogo de modelos que o renderer lê fica em
     `packages/sdk/src/llm/`. `packages/llm`, `packages/mood` e `packages/core-native` (addon napi)
     foram apagados em 2026-09-23 junto com o sidecar Node (catraca `check:sidecar-attrition` em 0).
   - Prompts vivem em markdown (`apps/desktop/prompts/<namespace>/*.md`, fora de `src/`) carregados
     em Rust (`modules/chat/prompt.rs`, `modules/brain/prompt_loader.rs`).
   - Toda fala da Mika é gerada por LLM.

---

## 3. Diretrizes de Produto e Código (Gamer Companion)

1. **Zero Mocks em Features:**
   - Toda feature e integração deve ser real e funcional de ponta a ponta. Proibido mock ou dados fictícios em código de produção.
2. **Bilateralidade UI ↔ Backend:**
   - Toda UI/botão/toggle precisa controlar uma função real e integrada no runtime (zero botões mortos).
   - Toda integração ou capacidade de backend deve ter interface visível ou configurável para o usuário.
3. **Mika Gamer Companion (Proativa & Sensorial):**
   - Escuta (STT), observa a tela (Vision), conversa (LLM) e reage proativamente ao contexto de jogos/desktop (puxa assunto com cooldown).
4. **Multiplataforma por Padrão:**
   - Suporte a Windows, Linux e macOS. Captura de tela, processos e atalhos devem respeitar o SO sem assumptions exclusivas de Windows.
5. **Simplicidade Extrema & Menos Linhas de Código:**
   - Menos linhas = menos bugs e manutenção mais simples. Soluções cirúrgicas, sem abstrações prematuras ou over-engineering.
   - **Complexidade Baixa:** Quebrar funções densas em funções puras menores (10 a 20 linhas), respeitando a complexidade cognitiva do Biome (<= 15).
   - **Zero Indireção Inútil:** Evitar camadas intermediárias e handlers que apenas repassam chamadas sem agregar validação ou transformação (achatar a chamada direta).
6. **Padrão Ouro de UI/UX (Mika > Personagem e Mika > Usuário):**
   - As telas de `character` e `user` são a referência canônica visual e de UX para todas as interfaces.
   - Padrão estrutural: Hero snapshot/cabeçalho no topo (identidade visual + fatos/status rápidos em pílulas), agrupamentos em `GlassSection` com ícone temático e descrição contextual, grid responsivo em `BentoGrid` com `GlassCard`, cabeçalhos trackeados (`uppercase tracking-[0.12em]`) e divisores visuais (`SectionDivider`).
   - Controles exclusivamente via `@mika/sdk-ui/settings` (`AuroraField`, `GlassSelect`, `GlassSlider`, `GlassToggle`, `StatusPill`, `StatTile`) com tokens de cor semânticos (zero literais).

---

## 4. Workflow de Validação

Antes de finalizar qualquer alteração, nesta ordem. Parou de passar, parou o trabalho:

1. `pnpm lint` — Biome limpo (0 erros).
2. `pnpm gates` — Bateria de invariantes verde.
3. `pnpm typecheck` — 0 erros de tipo.
4. Testes de unidade **somente onde houve alteração de código** — `vitest run <caminho>` dos
   arquivos/módulos tocados e `cargo test --lib -- <filtro>` dos módulos Rust tocados. Nunca a suíte
   inteira por conta própria; a suíte completa é o usuário quem pede.
5. `pnpm build` — Build de produção verde.
6. **E2E só quando o usuário solicitar.** Aí, só os specs dos fluxos pedidos
   (`playwright test -c e2e/playwright.config.ts e2e/specs/<spec>.spec.ts`), nunca `pnpm test` inteiro
   sem pedido explícito. Lembre: o E2E roda o binário release com a feature `e2e` (ganchos de teste, ausentes do binário distribuído) — `pnpm --filter @mika/desktop build:e2e` antes.
7. Com E2E solicitado e UI alterada, inspecione os screenshots em `apps/desktop/e2e/screenshots/`.

**MUST NOT** declarar trabalho pronto com qualquer um falhando. Se algo não passa por motivo alheio
à mudança, isso é **reportado**, não omitido.

**MUST**: invariante novo vem acompanhado de gate, ou da justificativa de por que não dá para
automatizar. Gate que só existe como script e não roda em nenhum passo acima não é gate.

**MUST**: quando um teste passa a falhar depois de uma correção, a primeira hipótese é que **o teste
documentava o defeito**. Leia o teste antes de reverter a correção — já aconteceu três vezes neste
repositório.

---

## 5. Política de Testes

Régua completa, padrões de lixo e fluxo de poda: skill `test-audit` (`.claude/skills/test-audit/`).
Carregue-a antes de escrever, revisar ou apagar qualquer teste.

1. **Teste antes do código, nunca depois.** Bug: primeiro o teste vermelho que reproduz, depois a
   correção. Feature: o teste descreve o comportamento antes da implementação. Teste de unidade
   escrito depois só para cobrir linhas ou subir `thresholds` é recusado — ele afirma o que o código
   já faz, inclusive o defeito.
2. **E2E é o mecanismo preferido para feature complexa** (voz, visão, chat com tools, UI, fluxo entre
   janelas). Unidade/contrato só para lógica pura ou contrato que o E2E não alcança de forma barata
   e determinística (ver §1 — a camada de unidade existe porque já achou defeito que o E2E não via).
3. **Todo spec E2E termina com um artefato verificável e repetível** — PNG em
   `apps/desktop/e2e/screenshots/` e/ou JSON com o estado afirmado —, gerado de forma determinística
   (LLM dublê, perfil isolado, sem rede externa) para que rodar de novo produza o mesmo resultado.
   Trace/vídeo só-em-falha não conta como artefato.
4. **Teste de baixo sinal sai.** Teste de unidade que não pegaria um defeito real que o E2E ou o dono
   de fronteira já não pegue é candidato a deleção — com a evidência exigida pela skill, um lote por
   fronteira, e nunca derrubando a catraca de cobertura (`thresholds`, `check:test-reach`) sem
   remedição justificada.

<!-- base-actions-main-policy -->
## GitHub Actions — padrão obrigatório (@main)

- Referencie as actions internas sempre pelo branch, nunca por SHA:
  - `uses: juninmd/base-actions/<action>@main`
  - `uses: juninmd/base-actions/.github/workflows/<arquivo>.yml@main`
- Não fixe `@<sha>` em `juninmd/base-actions`: a atualização é centralizada no
  `main` e passa a valer no próximo run.
- PRs cujos checks não iniciam por billing/spending limit são mesclados pelo bot central
  workflow `auto-merge-billing-blocked` do repositório `juninmd/base-actions` (em `.github/workflows/`).
  Para impedir o merge automático de um PR, aplique a label `do-not-merge`.