# 11 — Qualidade

## Fluxo obrigatório antes de submeter

Nesta ordem. Parou de passar, parou o trabalho.

```
pnpm install
pnpm lint        # biome
pnpm gates       # 42 gates de invariante (scripts/checks/) + test:rust (cargo test --lib), em pnpm gates
pnpm typecheck
pnpm test:unit   # unidade e contrato (Vitest), escopado ao que mudou — vitest run <caminho>
pnpm run build   # nunca marcar concluído sem build passar
pnpm test        # E2E (Playwright, apps/desktop/e2e) — só quando o usuário solicitar
```

> **Por quê:** este bloco já listou `pnpm test:contracts` e `pnpm smoke`, e a tabela abaixo já
> declarou treze gates `check:*`. **Nenhum desses scripts existia no `package.json`** — não havia
> `scripts/checks/`, e o que rodava de verdade era `lint`, `typecheck`, `build` e `test`. Todo
> invariante desta pasta ficou sem guarda enquanto a doc afirmava o contrário, o que é pior que não
> ter gate nenhum: lê como cobertura em review. Um fluxo obrigatório que cita comando inexistente é
> defeito, e é a mesma classe que `check:doc-paths` guarda para caminhos de arquivo.
>
> **Atualização (migração Tauri):** hoje `pnpm gates` já existe e roda de verdade — 40 scripts
> `check:*` mais `pnpm test:rust` (`cargo test --lib --manifest-path
> apps/desktop/src-tauri/Cargo.toml`) no fim da cadeia (`package.json`). `pnpm test:contracts` e
> `pnpm smoke` continuam sem existir como conceito de gate obrigatório — `smoke` existe no
> `package.json`, mas roda build + Playwright `--grep @smoke`, não é o mesmo fluxo. `AGENTS.md` (raiz
> do repo, fonte corrente) restringe o passo 4 a `vitest run <caminho>`/`cargo test --lib -- <filtro>`
> escopado ao que mudou, e o passo 6 (E2E) a "só quando o usuário solicitar" — a suíte completa deste
> bloco (`pnpm test:unit`/`pnpm test` sem escopo) é o piso de CI, não o que se roda a cada mudança.

**MUST NOT** declarar trabalho pronto com qualquer um falhando. Se algo não passa por motivo
alheio à mudança, isso é **reportado**, não omitido.

## Gates que protegem invariantes de spec

Todos rodam por `pnpm gates`. Cada script em `scripts/checks/` carrega no cabeçalho a história do
defeito que o originou — invariante sem história é o primeiro que alguém remove por achar que não faz
nada.

| Gate                           | Protege                                                                                                                                                                                                                                    |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `check:no-hardcoded-speech`    | fala sempre gerada por LLM ([07](07-persona-and-relationship.md))                                                                                                                                                                          |
| `check:renderer`               | isolamento de processo, e a ponte IPC como ponto único ([01](01-architecture.md))                                                                                                                                                          |
| `check:topology`               | roster ↔ pastas ([01](01-architecture.md))                                                                                                                                                                                                 |
| `check:file-naming`            | nome de arquivo segue AGENTS.md §2: `index` só para o barril, PascalCase só para componente React (`.tsx`) ([01](01-architecture.md)) |
| `check:boundaries`             | módulo só alcança a API pública de outro; núcleo não vê feature ([01](01-architecture.md))                                                                                                                                                 |
| `check:log-discipline`         | sem `console.*` em main/features ([10](10-observability.md))                                                                                                                                                                               |
| `check:events`                 | evento emitido tem listener, e listener tem emissor ([01](01-architecture.md))                                                                                                                                                             |
| `check:event-literals`         | evento do catálogo endereçado pela CONSTANTE, nunca por string crua ([01](01-architecture.md))                                                                                                                                             |
| `check:doc-paths`              | caminho de arquivo citado na doc existe — registro histórico é isento; README e esta spec citam todo `check:*` de `pnpm gates` e a contagem que declaram é a real                                                                          |
| `check:versions`               | `package.json`, `apps/desktop/package.json` e `src-tauri/Cargo.toml` com a mesma versão — o instalador e o "Propriedades" do exe não podem divergir                                                                                        |
| `check:external-links`         | link externo na UI sai por `open-external`, nunca `<a target="_blank">`/`window.open` — o webview do Tauri descarta o pedido sem erro ([09](09-settings-and-ui.md))                                                                        |
| `check:config-ui-consumed`     | "UI → runtime": controle de settings (`configPath` literal) que nenhum código de produção lê — controle morto ([09](09-settings-and-ui.md))                                                                                                |
| `check:dead-deps`              | dependência declarada que nenhuma linha do repositório importa                                                                                                                                                                             |
| `check:dormant-modules`        | módulo cujo único importador é o próprio teste — pronto e desligado                                                                                                                                                                        |
| `check:config-keys`            | chave de configuração que ninguém lê nem escreve — promessa não cumprida                                                                                                                                                                   |
| ~~`check:fetch-timeout`~~      | aposentado em 2026-09-23: `createLoggedFetch` saiu com o legado Node; a rede sai pelo Rust (`guarded_fetch`)                                                                                                                               |
| `check:pinned-downloads`       | URL de download no fonte nunca aponta para ref móvel (`master`, `resolve/main`, `HEAD`)                                                                                                                                                    |
| `check:external-http-logged`   | nenhuma saída de rede escapa do funil observado ([10](10-observability.md))                                                                                                                                                                |
| `check:privacy-chokepoint`     | redação de PII e autorização de rede na nascente, não no chamador                                                                                                                                                                          |
| `check:avatar-parity`          | os dois backends de avatar implementam o contrato inteiro ([12](12-avatar-render.md))                                                                                                                                                      |
| `check:expression-parity`      | paridade de expressões entre os backends de avatar ([12](12-avatar-render.md))                                                                                                                                                             |
| `check:packaged-assets`        | asset de terceiro fora do instalador, e inventariado                                                                                                                                                                                       |
| `check:license-policy`         | nenhuma dependência de produção (Rust, npm) com GPL/AGPL/LGPL-sem-exceção, licença não-comercial ou sem licença; `THIRD-PARTY-NOTICES.md` presente e com o hash de Cargo.lock + pnpm-lock + inventário manual de hoje (`pnpm notices:gen`) |
| `check:release-docs`           | todo `secrets.*`/`vars.*` lido em `.github/workflows` aparece em `steam/README.md`, e todo `STEAM_*`/`TAURI_*`/`APPLE_*`/`WINDOWS_*` citado ali é lido por algum workflow                                                                  |
| `check:prompt-files`           | todo `loadPrompt()` tem `.md`, e todo `prompts/` de módulo entra no bundle ([02](02-providers.md))                                                                                                                                         |
| `check:settings-nav`           | toda aba declarada tem rota no rail, e nenhuma seção de settings carrega uma aba só ([09](09-settings-and-ui.md))                                                                                                                          |
| `check:module-manifest`        | manifesto de módulo (deps/emits/consumes/config) desalinhado do código real, nos dois sentidos: uso não declarado ou declaração morta ([01](01-architecture.md))                                                                           |
| `check:module-ownership`       | todo arquivo de produção tem um módulo dono homologado — arquivo órfão fica fora de qualquer homologação ([01](01-architecture.md))                                                                                                        |
| `check:test-reach`             | catraca de quantos arquivos o teste de unidade alcança via import — só pode subir                                                                                                                                                          |
| `check:homologation-freshness` | relatório de homologação salvo que não bate mais com o artefato atual (árvore/contrato/deps mudaram), e módulo nunca homologado                                                                                                            |
| `check:ui-controls`            | controle de formulário cru (select/input/checkbox/range/switch) fora de `@mika/sdk-ui`, exigido por `AGENTS.md` §3.6                                                                                                                       |
| `check:ui-style`               | fonte abaixo do piso de legibilidade, cor crua (rgba/hsl/#hex) fora do tema, e `bg-accent`+`text-accent-text` (rosa sobre rosa)                                                                                                            |
| `check:ui-i18n`                | texto de UI escrito direto no código em vez de `t()`                                                                                                                                                                                       |
| `check:i18n-orphans`           | chave do dicionário de i18n que nenhum código cita                                                                                                                                                                                         |
| `check:dead-ui`                | peça de UI React sem importador de produção (só teste não conta)                                                                                                                                                                           |
| `check:tauri-shell-actions`    | toda ação pedida ao shell tem braço em `shell_actions.rs`, e todo braço declarado tem chamador                                                                                                                                             |
| `check:rust-logger`            | shell Tauri usa `log::error!`/`warn!` sem registrar um logger — a macro vira no-op silencioso sem um                                                                                                                                       |
| `check:tauri-custom-protocol`  | shell declara a feature `custom-protocol`; sem ela `tauri::is_dev()` fica `true` mesmo em `--release`; nenhum workflow (`.github/workflows/`) nem script de `steam/` empacota com `cargo build --release` em vez do CLI do Tauri           |
| `check:sidecar-attrition`      | catraca do sidecar Node — canais que ainda caem nele só podem diminuir, hoje em 0 (ver [01](01-architecture.md))                                                                                                                           |
| `check:rust-include-paths`     | `include_str!`/`include_bytes!` do Rust aponta para arquivo que existe — a maioria vive em `#[cfg(test)]`, onde `cargo check` não pega                                                                                                     |
| `check:rust-event-consumers`   | emissor/ouvinte de evento casados também do lado Rust ([01](01-architecture.md))                                                                                                                                                           |
| `check:config-schema-parity`   | "schema → lista": todo campo de `config/schemas/*.rs` aparece em algum `FEATURE_COMPONENT_HANDLED_FIELDS` ([09](09-settings-and-ui.md))                                                                                                    |
| `check:config-read-paths`      | "chamador → schema": todo `ConfigStore::get`/`set("raiz.folha")` do Rust corresponde a um campo real ([09](09-settings-and-ui.md))                                                                                                         |

**MUST**: gate cuja proteção depende de uma lista literal — o inventário de processo filho, as
nascentes de redação, o roster de features — confere a lista nos **dois** sentidos. Entrada sem
correspondente na árvore é entrada podre, e some da vista tão fácil quanto a que falta.

**MUST**: gate novo nasce com **probe negativo** — a violação que ele diz pegar, criada num arquivo
descartável, tem de fazê-lo sair 1. Sem isso não se sabe se ele pega alguma coisa; três gates desta
árvore estavam errados na primeira versão e só o probe mostrou.

> **Por quê:** o `check:external-http-logged` nasceu procurando `spawn(`/`execFile(` e deixava passar
> `exec()` — que roda com shell, é mais poderoso, e é justamente o que a leitura de janela ativa usa.
> Passou a detectar pelo import de `node:child_process` e apareceram **cinco** pontos de criação de
> processo que ele não via. Antes disso, a mesma versão usava `.test()` com regex global, que é
> stateful e alterna resultado a cada arquivo.

### Ainda não automatizados

Invariante escrito em spec sem gate é dívida declarada, não invariante esquecido. Estes seguem em
aberto:

- **[Resolvido]** `check:settings-fields` (schema → lista) e `config-read-paths` (chamador →
  schema): os dois nasceram como `check:config-schema-parity` e `check:config-read-paths`. Nem
  `check:ui-controls` nem `check:ui-i18n` cobriam esta pergunta — o primeiro pega controle cru fora
  do design system, o segundo pega texto fora de `t()`; nenhum dos dois compara o schema com a lista
  de campos por painel. Medido na árvore atual: `config-schema-parity` reconhece 239 campos de
  schema e achou 12 sem UI — 6 são "não se aplica" de verdade (estado de wizard/telemetria interno,
  isentos com motivo no próprio gate) e 2 já têm painel real (`AudioDevicesPanel.tsx`) que só não
  estava no dicionário `FEATURE_COMPONENT_HANDLED_FIELDS`; os 4 restantes são GAP de produto real,
  não corrigido aqui — `brain.enabled`, `mcp.enabled`, `person-catalog.enabled` e `browser.enabled`
  são toggles mestres de módulo sem controle em painel nenhum (`mcp.enabled` só é LIDO, para um
  badge de estado, em `ToolsMcpPanel.tsx`). `config-read-paths` mediu 69 caminhos únicos (108
  chamadas) em produção — todos válidos hoje.
- **[Resolvido pela remoção do legado Node]**: `createLLMProviderPlugin`/`LLMProviderRegistry` e
  `defineTool` não existem mais em `packages/sdk/src` — saíram com o SDK Node/runtime em
  2026-09-23 (ver [01](01-architecture.md), "Injeção de dependência"). O dead-code que este bullet
  descrevia foi removido junto com o mecanismo que o hospedava, não por limpeza dirigida a ele.
- **[Medido]** Redação de PII fora do domínio de tela: `person_catalog/pii.rs` (`redact_pii`/
  `redact_secrets`) é chamado de três nascentes — `rpc.rs` (painel de edição), `tool.rs` (tool-call
  do modelo) e `lifecycle.rs` (extração passiva de menção) — sem gate nenhum policiando um quarto
  chamador. `check:privacy-chokepoint` agora cobre as duas raízes (`person_catalog` e `character`);
  `character` não tinha, e continua sem ter, nenhuma chamada a `redact_pii`/`redact_secrets` — não há
  "redação por chamador" ali para proteger, mas qualquer chamada nova nesse módulo já cai sob o
  mesmo gate.

**MUST** (histórico — o TS não faz mais rede): todo cliente criado por `createLoggedFetch` declarava `timeoutMs` OU um `// sem-teto: <motivo>`
na linha de cima. O `fetch` do Node não tem timeout padrão, então o caso omisso é "pendurado para
sempre" — e a pessoa vê o "Testar conexão" girando sem fim. Stream de LLM, síntese de TTS por
`signal` do chamador, download de modelo e long-poll são isentos COM o motivo escrito; o que
`check:fetch-timeout` proíbe é a ausência de decisão.

**MUST**: saída de rede que não passa pelo funil normal chama `assertNetworkAllowed` manualmente
**antes** de tocar a rede — hoje em Rust (`apps/desktop/src-tauri/src/modules/desktop.rs`,
`apps/desktop/src-tauri/src/modules/features/desktop/launch.rs`,
`apps/desktop/src-tauri/src/modules/privacy/mod.rs`,
`apps/desktop/src-tauri/src/modules/features/privacy/mod.rs`), mantendo o mesmo nome do invariante
original. **[Medido]** `edge-tts` não existe mais como backend de TTS — nenhum provedor de voz o
usa (a lista de provedores hoje é `system`/`elevenlabs`/`openai-tts`/`azure-tts`/`google-tts`/
`piper`/`kokoro`/`supertonic`, ver `config/schemas/voice.rs`). O nome sobrevive só como entrada do
denylist de executáveis de rede (`network_executables()` em
`apps/desktop/src-tauri/src/modules/features/desktop/launch.rs`), a MESMA lista que bloqueia
`curl`/`wget`/`npm` quando o modo offline está ligado — ou seja, se algo tentasse invocar `edge-tts`
via `action=launch` (o tool que o modelo usa para abrir programas), cairia no mesmo
`assert_shell_launch_allowed` coberto por
`launch_tests.rs::shell_launch_guard_only_blocks_network_tools_when_offline_mode_is_on` (roda em
`pnpm test:rust`). O MUST original segue válido e coberto — não pelo nome `edge-tts` especificamente,
mas pela classe inteira de processo filho de rede, que é o que a lista protege.
`check:external-http-logged` não enxerga processos filhos, e foi exatamente por essa porta que o
modo offline vazou o texto de toda fala para a Microsoft, no design antigo (TS) que dava a cada
provedor seu próprio `spawn`.

**MUST**: invariante novo em spec vem acompanhado de gate, ou de justificativa de por que não dá
para automatizar.

**MUST**: todo gate da tabela roda em algum passo do fluxo obrigatório acima. Gate que só existe
como script não roda.

> **Por quê:** `check:no-hardcoded-speech` protege o invariante P0 do produto e não estava em
> nenhum passo do fluxo — só em `check:ci`. E cobria `tts.speak`, a chamada interna, mas não
> `speakThroughTTS`, o funil por onde todos os chamadores realmente passam. Um gate que guarda a
> camada errada lê como cobertura em review.

**MUST**: doc que aponta para arquivo que não existe é defeito, não imprecisão de estilo — a mesma
regra que trata comentário como não-evidência. `check:doc-paths` exige que o caminho citado exista
ou seja riscado (`~~caminho~~`, que é como se diz "isto foi apagado"). Registro histórico (livro-razão,
auditoria, homologação, checklist de migração) é isento por definição: ele cita a árvore da época, e
reescrevê-lo apagaria a evidência de quando o defeito existiu. A classe reincidiu duas vezes antes do
gate: 21 caminhos mortos nos briefs das trilhas de agente, e 37 na doc de produto — entre eles um
`pii-redactor.ts` que prometia redigir a IMAGEM antes do LLM e nunca existiu.

**MUST**: um gate irmão não pode ficar cego onde o outro enxerga. `check:events` casa emissor e
ouvinte pelo VALOR do evento, então um literal com typo dos DOIS lados forma um par consistente
entre si e invisível para o catálogo — por isso `check:event-literals` existe e exige a constante.

**MUST**: gate cuja pergunta depende de OBJETO em runtime é teste de contrato, não script de
`scripts/`. Texto não sabe que `EmbeddingSchema`, registrado sob a chave `embeddings`, declara
folhas sob a raiz `llm`; a primeira versão do `config-read-paths` como scanner textual acusou
`llm.embeddingTaskType`, que existe.

## O teste que quebra ao corrigir o bug

**MUST**: quando um teste passa a falhar depois de uma correção, a primeira hipótese é que **o
teste documentava o defeito**.

Leia o teste antes de reverter a correção.

> **Por quê:** já aconteceu três vezes seguidas neste repo. Exemplo desta rodada: um teste afirmava
> que o provedor `openai` lança "API key is missing". Depois de removê-lo do catálogo, passou a
> lançar "not supported". O teste estava certo sobre o passado e errado sobre o presente — a
> correção é reescrever a asserção para o contrato novo, não desfazer a mudança.

Corolário: **MUST NOT** relaxar uma asserção para "ficar verde". Ou o comportamento mudou de
propósito (reescreve o teste, documentando), ou a mudança está errada.

## Reproduzir antes de corrigir

**MUST**: bug tem reprodução antes da correção, e prova depois.

**MUST**: a reprodução acontece no ambiente **certo**.

> **Por quê:** ao investigar o diagrama sem rótulos, o primeiro repro rodou em DOM simulado e
> disse que a sanitização apagava o SVG inteiro — falso. Sem layout, a lib produzia um SVG de 16px
> e a serialização vinha vazia. Só o navegador real mostrou o defeito verdadeiro: rótulos
> removidos, caixas e setas intactas. Oráculo errado gera correção errada.

## Cobertura que importa

**SHOULD**: caminho infeliz tem teste — entrada malformada, provedor fora, credencial ausente,
timeout, cancelamento no meio.

**MUST**: todo invariante com `MUST NOT` que já foi violado tem teste de regressão.

## Validação em app real

**SHOULD**: mudança que afeta runtime é verificada com o app rodando, não só em teste.

**MUST**: para bug de log ou de custo, a prova é **quantitativa** — contagem de linhas, contagem
de requisições, antes e depois.

## Documentação

**MUST**: mudança funcional atualiza `/docs` e as specs afetadas **no mesmo commit**.

**MUST**: incidente que vira invariante entra na spec com a nota `Por quê:`. Invariante sem
história é o primeiro que alguém remove por achar que não faz nada.

## Segurança

**MUST NOT** commitar segredo. **MUST NOT** logar segredo.

**MUST**: dado de terceiro (catálogo de pessoas) fora de log, telemetria e relatório de erro.

**MUST NOT** forçar encerramento de processos do sistema durante validação.
