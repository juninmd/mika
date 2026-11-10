# 03 — Resiliência

**[Estado real pós-Tauri, atualizado em 2026-09-24]** O mecanismo original deste documento era do
`packages/llm` em TypeScript (`CircuitBreaker`/`HealthMonitor`/`llm-circuit-state.ts`) — apagado
inteiro em 2026-09-23 junto com o resto do sidecar Node. O substituto Rust
(`apps/desktop/src-tauri/src/modules/llm/provider_health.rs`, ligado a
`apps/desktop/src-tauri/src/modules/chat/send.rs`, `apps/desktop/src-tauri/src/modules/brain/turn.rs`
e, desde esta rodada, `apps/desktop/src-tauri/src/modules/vision/watcher.rs`) agora tem classificação
fatal/transiente, latch pré-I/O e cooldown/half-open para falha transiente — ver as seções abaixo
para o que mudou e o que ainda falta (sem retry/backoff PROGRAMADO em timer, e o funil ainda vive em
cada CHAMADOR, não dentro da resolução do `LanguageModel`, ver "Onde a guarda mora").

## O problema que esta spec resolve

Um provedor mal configurado ou fora do ar **não pode** virar um loop infinito de requisições
caras e logs idênticos. E o usuário **tem que saber** que algo está desligado, pela UI, não
garimpando o terminal.

> **Incidente de referência:** `vision.provider = openrouter` com `vision.model = "gemma3:4b"` (tag
> do Ollama). Cada ciclo do screen watcher subia ~140 KB de screenshot em base64 num request
> garantido HTTP 400, gerando 9 linhas de log. Para sempre. A UI dizia que a visão estava ligada.

## Duas classes de falha

**MUST**: toda falha de provedor é classificada antes de qualquer retry.

| Classe         | Sinal                                             | Exemplos                           | Política                                 |
| -------------- | ------------------------------------------------- | ---------------------------------- | ---------------------------------------- |
| **Fatal**      | `APICallError.isRetryable === false` e status 4xx | modelo inválido, key inválida, 404 | **Trava na 1ª falha**                    |
| **Transiente** | 429, 5xx, rede, timeout                           | rate limit, provedor caindo        | Circuit breaker com cooldown e half-open |

**MUST NOT** aplicar cooldown-and-retry a falha fatal.

> **Por quê:** um breaker clássico não resolve erro permanente. `gemma3:4b` nunca vira válido
> sozinho — half-open só religaria o loop de 140 KB a cada minuto, mais devagar.

**[Portado nesta rodada]** `provider_health::is_fatal_status` (`provider_health.rs`) classifica: 4xx
é fatal, EXCETO 429 (tabela acima) e ausência de status (rede/timeout, sempre transiente). O status
vem de `provider_health::extract_http_status`, que lê o texto de erro do `rig-core` (`CompletionError`
→ `http_client::Error::InvalidStatusCode*`, cujo `Display` é
`"HttpError: Invalid status code {StatusCode} with message: ..."`, pinado à versão `rig-core = "0.42"`
declarada em `Cargo.toml` — se o texto mudar numa atualização, a extração só volta a `None`
(transiente), nunca panica nem finge fatal). `record_failure` grava o resultado em
`HealthEntry::fatal` (sticky) e usa isso para decidir a política — ver "Latch fatal" abaixo.

## Latch fatal

**MUST**: o latch é chaveado por `provider:model`, não por provedor.

Consequência de desenho, não acidente: trocar o modelo nas configurações **destrava sozinho**,
porque a chave passa a ser outra. Não é preciso invalidação explícita.

**MUST**: o latch é desfeito quando muda config relevante (credencial, base URL) ou por ação
explícita do usuário na UI.

**MUST**: uma chamada bloqueada pelo latch falha **antes de qualquer I/O de rede**.

**[Estado real]** A chave `provider:model` sobrevive: `provider_health::HealthEntry` usa exatamente
essa chave (`key(provider, model)`, `apps/desktop/src-tauri/src/modules/llm/provider_health.rs`), e
trocar `llm.provider`/`llm.model` naturalmente aponta para outra entrada — a consequência de desenho
do MUST continua verdadeira por construção.

**[Portado nesta rodada]** Bloqueio antes do I/O: `chat::send::handle_chat_send`
(`apps/desktop/src-tauri/src/modules/chat/send.rs`) resolve a chave de saúde e chama
`provider_health::is_blocked` ANTES de montar o observer/histórico e chamar `turn::run_turn_llm` — um
par `disabled` devolve `blocked_by_health_response` sem que `run_turn_llm` seja sequer invocado
(zero I/O). Falha FATAL trava para sempre (`is_blocked` sempre `Some` até reset/sucesso/mudança de
config); falha TRANSIENTE libera sozinha depois de `TRANSIENT_COOLDOWN_MS` (60s, "half-open": uma
tentativa passa, se falhar de novo o cooldown recomeça).

**[Ainda vale como estava]** A "destrava" por mudança de config continua sendo só a CHAVE virar outra
(`provider:model`) — não existe invalidação explícita por handler de config cliqueada. Efeito prático
idêntico (a entrada velha nunca mais é lida), então o MUST original se sustenta por construção, sem
código dedicado.

## Onde a guarda mora

**MUST**: a guarda envolve o `LanguageModel` no ponto onde ele é **resolvido** — o funil por onde
passam chat, visão, roteamento e plugins de provedor.

**MUST NOT** exigir que cada chamador lembre de proteger a chamada.

> **Por quê:** existiam `CircuitBreaker` e `HealthMonitor` completos, testados e exportados no
> barrel do SDK, e **nenhum caminho de LLM os usava**. Um controle que existe no papel lê como
> seguro em review. Antes de acreditar que uma feature de custo ou segurança funciona: procure o
> chamador.

**[Estado real, atualizado nesta rodada]** O `packages/llm` que hospedava `CircuitBreaker`/
`HealthMonitor` foi apagado inteiro em 2026-09-23. O substituto Rust (`provider_health.rs`) continua
SEM um funil único no ponto de resolução do `LanguageModel` — cada chamador consulta
`provider_health::is_blocked`/grava `record_failure` por conta própria: `chat::send::handle_chat_send`
(chat) e, desde esta rodada, `vision::watcher::run_tick` (visão, via `suspend_for_provider_health`, ver
"Consumidores param de trabalhar" abaixo). A lição do "Por quê" (procure o chamador) agora tem os dois
chamadores que faltavam, mas o MUST de guarda CENTRALIZADA na resolução do `LanguageModel` continua em
aberto — um terceiro consumidor futuro (roteamento, plugins) ainda precisaria lembrar de chamar
`is_blocked` por conta própria.

## Consumidores param de trabalhar

**MUST**: um loop periódico cujo provedor está travado **suspende o próprio trabalho**, e não
apenas engole o erro por ciclo.

O screen watcher é o caso canônico: capturar tela sem ter para onde mandar a análise é CPU e
disco jogados fora. Ele para o timer e volta a ligar via evento de saúde.

**MUST**: a suspensão é logada **uma vez**, na transição — nunca por ciclo.

**[Portado nesta rodada]** `vision::watcher::run_tick` chama `suspend_for_provider_health` logo depois
de `should_tick` e ANTES da captura de tela — se `provider_health::is_blocked("ollama", vision_model)`
voltar `Some`, o tick inteiro é pulado (nem a tela é capturada). `run_tick` também alimenta o mesmo
registro: sucesso de `llm_client::analyze` chama `record_success`, e `VisionLlmError::Provider` chama
`record_failure` (com o status extraído do erro, mesma `extract_http_status` do chat). A chave é
sempre `("ollama", vision_model)` — `llm_client::analyze` só chama Ollama hoje (ver o cabeçalho
daquele arquivo), nunca `vision.provider` (que ainda não existe como seletor multi-provedor no lado
Rust, ver [02](02-providers.md)). A transição loga uma vez (`log::info!`/`log::warn!`), nunca por
ciclo — `suspended: &mut bool`, mesmo padrão de `last_hash`.

## Superfície de UI

**MUST**: existe evento de saúde de provedor no catálogo do EventBus, e a UI mostra:

- qual par provedor+modelo está desativado;
- o **motivo cru devolvido pelo provedor** (é o texto que permite o usuário agir) e o status HTTP;
- que as chamadas estão sendo bloqueadas antes de sair;
- um botão de tentar de novo.

**MUST NOT** representar provedor desativado apenas como linha de log ou como erro genérico.

O aviso é global, não de uma aba: aparece em qualquer seção de Settings.

**[Estado real, atualizado nesta rodada]** `ProviderHealthBanner`
(`apps/desktop/src/renderer/src/components/settings/ProviderHealthBanner.tsx`) mostra `provider`+
`model` de todo par com `state === 'disabled'`, o `reason` cru vindo do provedor, `statusCode` quando
existe, e um botão "Tentar de novo" (`resetProviderCircuit`) — montado globalmente
(`SettingsPanelContent.tsx`/`App.tsx`). Com o latch pré-I/O portado nesta rodada, "bloqueando antes de
sair" agora É literalmente verdadeiro para chat e visão: `is_blocked` impede a chamada antes de
`run_turn_llm`/`llm_client::analyze` serem sequer invocados. `HealthEntry` ganhou o campo `fatal`
(exposto em `to_json()` como `"fatal"`), e o renderer agora distingue as duas classes visualmente:
`providerHealthPresentation` (`ProviderHealthBanner.tsx`) mapeia `fatal` para um `StatusPill`
(`@mika/sdk-ui/settings`) vermelho + `shell.providerHealth.fatalBadge`/`fatalHint` ("ação necessária,
não tenta de novo sozinho") e `!fatal` para o pill amarelo + `transientBadge`/`transientHint`
("tentando de novo sozinho após o cooldown"), com as 7 traduções em `packages/sdk/src/i18n/locales/`.
Teste: `providerHealthPresentation` em
`apps/desktop/src/renderer/src/components/settings/__tests__/provider-health-presentation.test.ts`
(lógica pura, sem montar DOM — o ambiente do vitest deste repo é `node`, cobertura de tela é E2E).

## Disciplina de log

**MUST**: transições de estado logam uma linha. Tentativas individuais não logam.

Uma falha permanente produz exatamente:
1. uma linha do erro real, com status e motivo;
2. uma linha de "provedor desativado";
3. uma linha de "consumidor suspenso".

Depois disso, **silêncio** até a config mudar.

**[Portado nesta rodada]** `chat::send::handle_chat_send` e `vision::watcher::run_tick` agora emitem
`log::warn!("... desativado (...)")` exatamente na transição para `disabled` (checando
`entry.to_json()["state"]` depois de `record_failure` — como o latch bloqueia qualquer nova falha
antes de rede a partir daí, isto só volta a disparar depois de um reset/sucesso/mudança de config,
nunca por tentativa). A "linha do erro real" ainda é o `err.to_string()` publicado em
`EventKey::ChatError`/o `log::debug!` de análise de visão, não uma linha `log::` dedicada — ver
[10](10-observability.md) para o estado geral de log do shell Rust. "Consumidor suspenso" é o
`log::info!` de `suspend_for_provider_health` (visão); chat não tem um "consumidor" no sentido de
loop periódico — o latch pré-I/O já cobre o equivalente (nenhuma chamada nova até resolver).

## Métrica de aceitação

Com um provedor permanentemente quebrado e o app rodando por 1 hora:

- requisições HTTP para aquele par: **exatamente 1**
- linhas de log mencionando a falha: **≤ 3**
- estado visível na UI: **desativado, com motivo**

**[Portado nesta rodada]** Com o latch pré-I/O, "exatamente 1" agora se sustenta para o caso FATAL: a
1ª chamada descobre a falha (única requisição HTTP que sai), `record_failure` marca `disabled` na
hora (`is_fatal_status`), e toda chamada seguinte é barrada por `is_blocked` antes de qualquer rede —
para sempre, até reset/sucesso/mudança de config. Para falha TRANSIENTE, o número passa a ser
`MAX_FAILURES_BEFORE_DISABLED` (3) requisições para desativar, e depois disso UMA requisição de
sondagem a cada `TRANSIENT_COOLDOWN_MS` (60s, half-open) — não mais uma por turno de chat
indefinidamente, mas também não é "exatamente 1" nesse caso (a spec original não distinguia as duas
classes quando escreveu esta métrica; o número exato agora depende da classe).
