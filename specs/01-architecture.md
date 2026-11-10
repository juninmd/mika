# 01 — Arquitetura

## Forma do sistema

**[Migrado Electron → Tauri, Stage C, 2026-09-22]** Um app Tauri 2. Um shell Rust. Um pacote de
SDK (contratos/tipos/i18n). Sem backend próprio, sem plugins externos, sem descoberta automática.
`src/main` e o backend Node de `src/features/*` foram apagados; o sidecar Node que ainda restava
para os canais não portados foi apagado em 2026-09-22, com a catraca `check:sidecar-attrition` em 0
(`apps/desktop/src-tauri/src/bridge.rs`: canal sem dono nativo agora é erro nomeado, "o sidecar Node
não existe mais", não mais um fallback).

```
Rust shell (src-tauri)  ←→  bridge (invoke/listen)  ←→  Renderer (React)
     │
     ├── modules/<ctx>/          núcleo bloqueante (brain, browser, chat, inference, llm, memory,
     │                           telemetry, voice, vision)
     └── modules/features/<ctx>/ features tardias (character, desktop, mood, person_catalog,
                                  privacy, scheduler, user — `gamer` e `mcp` são frentes próprias em
                                  `modules/gamer/` e `modules/mcp/`, fora do roster de
                                  `modules/features/mod.rs`, ver seção "O roster é a governança")
```

`apps/desktop/src/renderer/bridge/` (ex-preload) expõe o mesmo `window.electronAPI` de sempre —
`index.ts` chama `contextBridge.exposeInMainWorld`, e os canais usam `ipcRenderer.invoke`/`.on` —
mas os dois símbolos vêm de um shim (`apps/desktop/src/shared/tauri-renderer-runtime.ts`) que
fala `invoke`/`listen` do Tauri por baixo. O contrato do renderer não mudou; o transporte sim.

## Onde uma capacidade mora

A decisão é feita pelo **ciclo de boot**, não por gosto:

- Precisa estar de pé **antes do primeiro paint**? → `modules/<ctx>/` (Rust), registrado em
  `apps/desktop/src-tauri/src/native/mod.rs` / `lib.rs::setup`.
- Pode subir **depois da janela aparecer**? → `modules/features/<ctx>/` (Rust), roster em
  `modules/features/mod.rs`. A UI correspondente (quando existe) mora em
  `apps/desktop/src/features/<ctx>/renderer/settings.tsx`, registrada em
  `apps/desktop/src/renderer/src/lib/feature-ui-registry.ts` — é o que sobra de
  `apps/desktop/src/features/<ctx>/` em TypeScript; o `<ctx>.module.ts` de cada uma (ex.
  `apps/desktop/src/features/character/character.module.ts`) hoje é só manifesto de posse do
  `renderer/`, não mais um módulo com serviços/tools Node por trás.

**MUST**: uma capacidade mora em **um** lugar só, com **uma** responsabilidade. Se a descrição da
pasta precisa de "e", são dois módulos.

**MUST**: fronteira de módulo é API pública — em Rust, o `mod.rs`/`pub fn` do módulo (nunca
`use` direto de dentro de outra feature; ver comentário de `modules/features/mod.rs`: "núcleo nunca
importa de feature. O caminho de volta é token de DI tolerando ausência, nunca `use` direto"); no
que resta em TypeScript, `index.ts` do módulo. Nunca caminho relativo para dentro de outro módulo.
Gate: `pnpm check:boundaries`.

## O roster é a governança

Toda feature existe em runtime porque alguém **escreveu à mão** em duas listas:

- `apps/desktop/src-tauri/src/modules/features/mod.rs` (`pub mod <feature>;`) — roster do backend Rust; pasta sem registro é erro de compilação
- `apps/desktop/src/renderer/src/lib/feature-ui-registry.ts` — roster de UI (se tiver UI)

`gamer` (gamer-companion) e `mcp` **não** passam por este roster — vivem em `modules/gamer/` e
`modules/mcp/` como frentes próprias, à parte de `modules/features/mod.rs` (comentário do próprio
arquivo: "mais `mcp`/`gamer-companion` (frentes próprias, fora daqui)"). É uma exceção ao "toda
feature está numa lista literal" ainda sem justificativa escrita nem gate próprio — o mesmo tipo de
lacuna que motivou este invariante.

**MUST NOT** haver descoberta automática de features.

> **Por quê:** o catálogo anterior era derivado (interseção de pastas com deps do `package.json`).
> Faltar de um lado fazia a capacidade não existir — sem erro, sem log. Uma feature de tier `core`
> ficou dormente por semanas, e **8 features estavam nesse estado** quando a arquitetura mudou.
> Uma lista literal não deriva de nada, logo não pode divergir de nada.

`pnpm check:topology` **MUST** falhar se pasta e lista divergirem em qualquer direção.

## Boot degradado é um estado de primeira classe

`mika:get-health` reflete prontidão real:

| Estado         | Significa                                  |
| -------------- | ------------------------------------------ |
| `initializing` | core mínimo ainda não subiu                |
| `starting`     | core pronto, fase deferred rodando         |
| `ok`           | core + features prontos, **nenhuma falha** |
| `degraded`     | qualquer feature falhou ao ativar          |

**MUST**: uma única feature que não sobe já torna o boot `degraded`.

> **Por quê:** rodar com menos do que foi embarcado não é saudável. Uma feature falhou em todo boot
> por meses enquanto o health respondia `ok` — a única evidência era uma linha de log que ninguém
> lia.

**[Aplicado, 2026-09-24]**: `mika:get-health` (Rust, `apps/desktop/src-tauri/src/modules/health.rs`)
detecta falha de registro por feature. `modules/mod.rs::register_config_bound_tools` só logava
(`log::error!`) cada `Err` de `person_catalog::register_tools`/`register_rpc`,
`memory::social::register_rpc`, `voice::register`/`register_voice_tool`, `privacy::register`,
`scheduler::register` e a resolução do `ConfigStore`; agora cada um chama
`crate::feature_health::record_registration_failure(feature, error)` (`feature_health.rs`, novo —
um `Vec<String>` protegido por `Mutex`, sem acoplar `modules/mod.rs` a `health.rs`). `health()` lê
`crate::feature_health::registration_failures()`: lista vazia mantém `status`/`bootstrap` como
antes; qualquer entrada vira `status: "degraded"` e `bootstrap.degraded: true` com a lista em
`bootstrap.errors` — é o campo que `BootHealthBanner.tsx` já checava
(`health?.status !== 'degraded'`) sem que o backend jamais emitisse esse valor. Lógica extraída em
`bootstrap_json` (pura, sem `AppHandle`) e coberta por
`modules::health::tests::bootstrap_json_{not_degraded_when_no_registration_errors,degraded_when_one_feature_fails}`.

**[Aplicado, 2026-09-24]**: a primeira passada (`modules::all()`, chamada por `native::registry()`
antes de existir `AppHandle`) não tem `Result` nenhum para propagar — cada `<ctx>::modules()` só
constrói structs e registra tools infalíveis (`ToolRegistry::register`/`register_stub`). A falha
real que passava por ali era outra: `ToolRegistry::register` (`modules/tools/registry/mod.rs`) já
rastreava colisão de nome inesperada (dois módulos registrando a mesma tool sem passar por
`register_stub`) num `unexpected_collisions: Vec<String>` PRÓPRIO, sem ninguém fora do registry
conseguir ver isso — só um `log::warn!` que ninguém lê, a mesma classe de defeito que motivou este
invariante. `register`/`register_stub` agora chamam
`crate::feature_health::record_registration_failure("tool_registry", ...)` no mesmo branch que já
alimentava `unexpected_collisions`, então uma colisão real de nome na primeira passada também vira
`bootstrap.degraded` — coberto por
`modules::tools::registry::tests::unexpected_collision_reaches_feature_health`.

## Isolamento de processo

**MUST NOT** importar `fs`, `path`, `child_process` ou qualquer lib Node no renderer. Toda
comunicação passa por `window.electronAPI` — a ponte em
`apps/desktop/src/renderer/bridge/` (mesmo contrato de sempre, ex-preload), hoje falando
`invoke`/`listen` do Tauri por baixo (ver "Forma do sistema").

**MUST**: toda rota IPC valida o payload na fronteira antes de agir. Rotas sensíveis também são
gateadas pela janela de origem, com `assertOwnerSurface`/`is_owner_surface` (Rust,
`apps/desktop/src-tauri/src/shell_actions.rs`).

**[Fechado no porte Tauri]**: `bridge.rs` continua sem um router/Zod único (não há um ponto central
que garanta a validação para toda rota) — a validação é por handler, um comando Rust desserializando
e conferindo o próprio payload. O que fechou nesta rodada é a CONTABILIDADE: todo canal reivindicado
em produção (`bridge.rs::SHELL_CHANNELS` + `NativeModule::channels()` de cada um dos 14 módulos em
`crate::modules::all()`) tem uma entrada revisada em
`apps/desktop/src-tauri/src/ipc_validation_audit.rs`, com veredito nomeado —
`Validated(razão)` apontando onde o handler valida, ou `Allowlisted(razão)` quando o payload é
vazio/ignorado ou o pior caso é inofensivo (leitura sem efeito colateral). O teste
`every_active_channel_has_an_audit_entry` reprova um canal novo sem entrada; sem ele, o gap ficava
invisível — compilava, rodava, e só era notado por leitura manual.

A auditoria desta rodada achou (e fechou) dois canais realmente sem teto: `mika:voice-transcribe`
aceitava `audioBase64` de qualquer tamanho (agora `MAX_AUDIO_BASE64_CHARS`, ~5min de PCM16 mono
16kHz, espelhado em `desktopTranscribeRequestSchema.audioBase64.max()`) e `voice:vad-audio` aceitava
`samples` de qualquer tamanho por chamada — um canal chamado até 100 Hz (agora
`MAX_SAMPLES_PER_CALL`, 1s @ 16kHz, espelhado no `.refine()` de `desktopVadAudioPayloadSchema`). Os
demais canais já validavam tipo/forma na fronteira (owner-surface, enum fechado, schema de
`ConfigStore`, ou regex de path) — ver o comentário de cada entrada em `ipc_validation_audit.rs`.

**MUST**: renderer importa apenas de `@mika/sdk-ui` e `@mika/sdk/contracts`
(`packages/sdk/src/contracts/desktop-ipc.ts`).

**[Removido no porte Tauri]**: `@mika/sdk/node` e o `container`/`resolve()` de DI que ele expunha
foram apagados em 2026-09-23 junto com o legado Node do SDK — não existe mais "entrypoint que arrasta
Node para o browser" porque não há mais entrypoint Node nenhum no SDK.

Gate: `pnpm check:renderer` (`scripts/checks/renderer.mts`). O antigo `pnpm audit:ipc` saiu do
`package.json` com o Electron.

## Comunicação entre módulos

**MUST**: cross-module via EventBus. O barramento em si é Rust
(`apps/desktop/src-tauri/src/events/` — `bus.rs`, `keys.rs`), com as chaves espelhadas no catálogo
TypeScript (`packages/sdk/src/svc/events.ts`) que o renderer consome. `check:events` e
`check:event-literals` seguem exigindo a constante do lado TS; `check:rust-event-consumers`
(`scripts/checks/rust-event-consumers.mts`) é o equivalente que casa emissor/ouvinte do lado Rust.

**MUST NOT** emitir evento que ninguém escuta como forma de "integrar".

> **Por quê:** o gate de aprovação de ferramentas emitia `APPROVAL_REQUIRED` — um evento com **zero
> listeners** — depois que a ferramenta já tinha executado. Parecia integrado em review e em docs.

Antes de aceitar qualquer evento novo: **quem escuta?** Sem resposta, não entra.

## Injeção de dependência

**MUST**: dependências chegam por parâmetro ou por estado gerenciado, nunca por singleton importado
no topo do módulo. Em Rust, o estado do backend é gerenciado pelo próprio Tauri
(`app.manage(ShellState::new())`, `apps/desktop/src-tauri/src/lib.rs`), e cada módulo recebe o
`AppHandle`/estado por parâmetro.

**[Removido no porte Tauri]**: o mecanismo antigo, em que as dependências chegavam pelo `ctx` do
módulo com `resolve()`/`container` expostos só por `@mika/sdk/node`. Esse runtime de DI foi apagado
em 2026-09-23 junto com o legado Node do SDK.

## Ciclo de vida

**MUST**: todo timeout, interval, listener ou processo que um módulo inicia é destruído quando o
módulo desliga. Sem exceção.

**[Aplicado, 2026-09-24]**: não existe um `activate()`/`deactivate()` formal por módulo — o
substituto é um sinal de desligamento de PROCESSO, `apps/desktop/src-tauri/src/shutdown.rs`
(`tokio::sync::watch<bool>` atrás de um `OnceLock`, não `tokio-util::CancellationToken`: `tokio` já
é dependência direta com a feature `sync`, e um `watch` dá exatamente o que falta — um disparo,
lido por N assinantes, inclusive um assinante tardio que chega depois do disparo). `lib.rs`
(`RunEvent::ExitRequested`) chama `shutdown::trigger()` uma vez, ao lado do
`modules::mcp::disconnect_all()` que já matava o processo filho do MCP. Todo timer de tick e todo
assinante do `EventBus` de `modules/` (22 sites — tick de `awareness`/`power`/`log_stream`/
`active_window`/`mood::boot`/`game_watcher`/`clipboard_watch`/`vision::watcher`, e os assinantes de
`hud_bridge`/`thought`/`post_turn::listener`/`assistant_bridge`/`expression_bridge`/
`affinity_notice`/`person_catalog::lifecycle`/`game_learning`/`game_partners`/
`clipboard_analysis`/`auto_speak`/`bus_bridge`/`expression_sync`/`awareness`) agora faz
`tokio::select!` contra `shutdown::until_triggered`, quebrando o loop no disparo. Um `spawn`
limitado por natureza (`features::character::expression_action::run_expression`: um `sleep` de no
máximo 30s e depois reverte, nunca reagenda) segue sem o sinal — não é o `loop`/assinante que este
MUST cobre.

Provado em duas camadas: `shutdown.rs::tests` (o mecanismo em si, com um `watch::channel` local por
teste — nunca o canal de processo, que não tem `reset()` de propósito) e
`modules::lifecycle_shutdown_gate` (leitura de fonte: varre todo `.rs` de `modules/`, isola o corpo
de cada `tokio::spawn`/`tauri::async_runtime::spawn` por contagem de chaves — não uma janela fixa,
que vazaria para um `#[cfg(test)] mod tests` vizinho com `while let` sem relação — e reprova se um
`loop {`/`while let` não referenciar `shutdown::`; `ALLOWLIST` interna, vazia hoje, é a única forma
de declarar uma exceção, com motivo).

## Contrato de ferramentas LLM

Toda tool retorna `{ success: boolean, data?: unknown, error?: string }` — hoje declarado em Rust
(`apps/desktop/src-tauri/src/modules/brain/tools/*.rs`, ex.: `content.rs`, `web.rs`,
`collaboration.rs`, `match_effect.rs`, `memory_console.rs`; e o gate de aprovação em
`modules/tools/approval.rs`, que devolve `{ success: false, error: 'user_denied' }` em qualquer
decisão que não seja `approved: true`). Nunca lança para o loop de ferramentas.
