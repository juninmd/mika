# AGENTS.md

Mika: companheira VTuber com IA para games, código e cultura nerd. Roda local no PC do usuário (Windows, Linux e macOS). Textos de produto em pt-BR.

## Stack
- Tauri 2 (backend em Rust, `apps/desktop/src-tauri`), Node 24, pnpm 10 (`packageManager`)
- Renderer: React 19, Tailwind 4, TypeScript 7; lint e formatação com Biome 2
- Testes: Vitest 5 (unidade e contrato), Playwright 1.64 (E2E)
- Electron foi removido na migração Tauri (2026-09-22). Não reintroduza.
- Renderer é só UI e a ponte `apps/desktop/src/renderer/bridge`; lógica de backend vive em Rust

## Commands
| Comando | O que faz |
|---|---|
| `pnpm lint` | Biome (`biome check .`) |
| `pnpm typecheck` | Tipos em todos os workspaces |
| `pnpm gates` | Invariantes em `scripts/checks/` + `test:rust` |
| `pnpm test:rust` | `cargo test --lib` do shell Tauri |
| `pnpm test:unit` | Vitest com catraca de cobertura (`thresholds` em `vitest.config.mts`, só sobe) |
| `pnpm build` | Prompts e renderer (`vite.tauri.config.ts`) |
| `pnpm --filter @mika/desktop tauri:build` | Instalador nativo |
| `pnpm test` | Suíte E2E completa (`apps/desktop/e2e`); só com pedido explícito |
| `pnpm dev` | `tauri dev` |
| `pnpm live2d:gen` | Regenera motions do avatar |

Execute comandos em modo silencioso (`--quiet`, `--silent`) para poupar tokens.

## Structure
- `apps/desktop/src-tauri/src/modules/<ctx>/`: backend Rust. Núcleo: `brain`, `browser`, `chat`, `inference`, `llm`, `memory`, `telemetry`, `voice`, `vision`. Contextos próprios: `gamer/`, `mcp/`. Features tardias: `features/<ctx>/`, registradas em `features/mod.rs` (pasta sem `mod` é erro de compilação)
- `apps/desktop/src/features/<ctx>/renderer/`: UI de cada feature, registrada em `apps/desktop/src/renderer/src/lib/feature-ui-registry.ts`
- `apps/desktop/prompts/<ns>/*.md`: prompts em markdown, fora de `src/`
- `apps/desktop/e2e/`: specs, fixtures e screenshots E2E
- `packages/sdk/`, `packages/sdk-ui/`: contratos, tipos, i18n e UI compartilhada
- `scripts/checks/`: gates de invariante; `specs/`: especificações
- `.claude/skills/test-audit/`: régua e fluxo de poda de testes

## Architecture invariants
- Renderer não importa Node (`fs`, `path`). Fala com o backend só por `window.electronAPI`, a ponte `apps/desktop/src/renderer/bridge/`
- Canais e rotas tipados em `packages/sdk/src/contracts/desktop-ipc.ts`. Rotas sensíveis chamam `assertOwnerSurface` (Rust)
- Comunicação entre módulos só por eventos tipados: barramento Rust em `apps/desktop/src-tauri/src/events/`, chaves espelhadas em `packages/sdk/src/svc/events.ts`. Sem container DI em TypeScript
- Backend Rust nunca lê TypeScript em runtime; só `include_str!`/`include_bytes!` de fonte compartilhada (`check:rust-include-paths`)
- STT (Whisper) e TTS (Supertonic/Kokoro) rodam em Rust (`modules/inference/`). Backend aceita só PCM16 mono 16 kHz; decodificação de áudio fica no renderer
- LLM local padrão: Ollama. Provedores remotos em Rust (`modules/llm.rs`, `modules/brain/`). Catálogo de modelos em `packages/sdk/src/llm/`
- Toda fala da Mika é gerada por LLM
- Nomes: arquivos em kebab-case; sufixos com ponto (`.service.ts`, `.runtime.ts`, `.schema.ts`, `.feature.ts`, `.module.ts`), nunca hífen (`chat-service.ts` está errado). PascalCase só para componente React. Nunca `index.<algo>.ts`; `index` é o barril
- `prompt-loader.ts` de cada módulo não se move: ele resolve a pasta de prompts pelo próprio diretório, e mover faz o prompt sumir sem erro

## Product rules
- Zero mock ou dado fictício em código de produção. Toda feature e integração é real de ponta a ponta
- Zero botão morto: toda UI controla função real no runtime
- Toda capacidade do backend tem interface visível ou configurável para o usuário
- Suporte a Windows, Linux e macOS. Captura de tela, processos e atalhos não assumem um SO só
- Mika é proativa e sensorial: escuta (STT), observa a tela (Vision), conversa (LLM) e puxa assunto com cooldown
- Simplicidade: funções de 10 a 20 linhas, complexidade cognitiva <= 15 (Biome), sem indireção que só repassa chamada
- UI de referência: telas `character` e `user`. Hero no topo, `GlassSection`, `BentoGrid` com `GlassCard`, cabeçalhos `uppercase tracking-[0.12em]`, `SectionDivider`
- Controles só via `@mika/sdk-ui/settings` (`AuroraField`, `GlassSelect`, `GlassSlider`, `GlassToggle`, `StatusPill`, `StatTile`), com tokens de cor semânticos, sem literais

## Testing
- Teste antes do código. Bug: primeiro o teste vermelho que reproduz, depois a correção. Teste escrito depois só para cobrir linhas ou subir `thresholds` é recusado
- E2E é a prova válida para UI e para o app de verdade. Unidade e contrato (Vitest) são para lógica pura
- Todo spec E2E termina com artefato determinístico e repetível: PNG em `apps/desktop/e2e/screenshots/` e/ou JSON com o estado afirmado. Trace ou vídeo só em falha não conta
- Teste de baixo sinal sai. Carregue a skill `test-audit` antes de escrever, revisar ou apagar teste
- Nunca reduza `thresholds` de cobertura nem desligue gate para passar
- Se um teste passa a falhar após uma correção, a primeira hipótese é que o teste documentava o defeito. Leia o teste antes de reverter a correção

## Validation workflow
Na ordem, e parou de passar, parou o trabalho:
1. `pnpm lint`
2. `pnpm gates`
3. `pnpm typecheck`
4. Unidade só dos arquivos tocados: `vitest run <caminho>` e `cargo test --lib -- <filtro>`. A suíte inteira só se o usuário pedir
5. `pnpm build`
6. E2E só quando o usuário pedir, e só os specs pedidos: `playwright test -c e2e/playwright.config.ts e2e/specs/<spec>.spec.ts`, depois de `pnpm --filter @mika/desktop build:e2e`
7. Com E2E e UI alterada, abra os screenshots em `apps/desktop/e2e/screenshots/`. Gate verde não prova tela

Nunca declare pronto com algo falhando. Falha por motivo alheio à mudança é reportada, não omitida. Invariante novo vem com gate, ou com a justificativa de por que não dá para automatizar.

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
