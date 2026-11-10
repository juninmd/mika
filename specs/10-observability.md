# 10 — Observabilidade

**[Estado real pós-Tauri, atualizado em 2026-09-24]** O logger de dois tiers (`logger.human?.()`,
arquivos `mika-human.log`/`mika-ai.ndjson`) era do processo Node (`src/main`), apagado no Stage C da
migração (2026-09-22). O shell Rust usa o crate `log` + `tauri_plugin_log`
(`apps/desktop/src-tauri/src/lib.rs`), com um ring buffer em memória
(`modules/system/log_buffer.rs`) alimentando o painel. Ao longo de duas rodadas, as lacunas abertas
por essa simplificação foram fechadas, uma a uma, dentro do MESMO desenho de sink único ganhando
peças, não recriando o processo Node: `audience` deixou de ser `"human"` fixo e passou a derivar do
nível (`log_buffer.rs::audience_for`); `traceId` ganhou um equivalente Rust via `tokio::task_local!`
(`modules/system/log_trace.rs`), amarrado ao turno em `chat/send.rs`; `token_usage.rs` passou a
gravar também por `(surface, provider, model)`, não só o total agregado; e, nesta rodada
(`modules/system/log_files.rs`), os dois ARQUIVOS voltaram a existir de verdade —
`logs/mika-human.log` e `logs/mika-ai.ndjson`, escritos a partir do mesmo `LogRecord` já redigido e
com `audience`/`traceId` resolvidos, fora do caminho quente do `log::log!` (canal `mpsc` + task de
fundo, nunca I/O de disco na chamada) — e um `human_log!` (`modules/system/human_log.rs`) marca os
call sites de decisão/sintoma que a spec pede em frase-com-efeito. As seções abaixo marcam o que
segue sem objeto (revogado pelo porte) e o que é lacuna real (não portado).

## Para que serve o log

Para você abrir o terminal depois de algo estranho e **entender em 30 segundos**. Se o log só serve
para provar que o código executou, ele é ruído.

## Dois públicos, dois tiers

Um log só servia mal aos dois leitores: prosa demais para a máquina, ruído demais para a pessoa.

| Tier    | Quem lê    | Onde                                      | O que entra                                                 |
| ------- | ---------- | ----------------------------------------- | ----------------------------------------------------------- |
| `human` | a pessoa   | terminal + `logs/mika-human.log` + painel | só momentos que exigem uma decisão ou explicam um sintoma   |
| `ai`    | agente/dev | `logs/mika-ai.ndjson`                     | **tudo**, uma linha JSON por evento, com `traceId` e `meta` |

**[Reconstruído, 2026-09-24]** Os dois arquivos existem de novo, com objeto real —
`modules/system/log_files.rs` (novo). O que existe hoje (`apps/desktop/src-tauri/src/lib.rs`, plugin
`tauri_plugin_log` + sink próprio):
- **stderr** do processo shell (`tauri_plugin_log::TargetKind::Stderr`);
- **`<dados-do-app>/logs/mika-human.log`** — só o subconjunto `audience == "human"` (deriva do
  nível, ver `audience_for` abaixo), uma linha de texto (`LogRecord::formatted`) por evento;
- **`<dados-do-app>/logs/mika-ai.ndjson`** — TUDO, uma linha JSON (`serde_json::to_string(&LogRecord)`)
  por evento, redigida e com `audience`/`traceId` já resolvidos;
- um **ring buffer em memória** de 500 entradas (`apps/desktop/src-tauri/src/modules/system/log_buffer.rs`,
  `MAX_LOGS`) que alimenta o painel de Logs de Settings via `system:getRecentLogs` (carga inicial) e um
  lote periódico drenado por `apps/desktop/src-tauri/src/modules/system/log_stream.rs`.

O antigo target `TargetKind::LogDir { file_name: "mika-shell" }` saiu: ele escrevia direto do `fern`,
ANTES da redação de `log_redact.rs` — uma chave vazando por acidente numa mensagem chegaria a esse
arquivo em texto puro, mesmo com o ring buffer redigindo corretamente. Os dois arquivos novos nascem
do MESMO `LogRecord` que o ring buffer já usa (`log_buffer::push_from_log_record` devolve o registro
cru; `lib.rs` repassa a `log_files::enqueue`), então essa lacuna de redação fecha como efeito
colateral, não como correção separada. Fora do caminho quente: `enqueue` só empurra num `mpsc`
não-limitado; quem abre os arquivos e escreve é uma task de fundo (`log_files::spawn`, mesmo padrão
de `log_stream::spawn`) — nenhum `log::log!` do processo passa a esperar por disco. Rotação: teto de
10 MiB por arquivo, truncado para vazio ao estourar (não é `RollingFileAppender`; ver o cabeçalho de
`log_files.rs` para o porquê de um design tão pequeno bastar aqui). Diretório resolvido por
`state::app_data_dir` (respeita `MIKA_PROFILE_DIR`, mesma regra de todo outro sink de arquivo do
processo). Cobertura: `modules::system::log_files::tests::{a_human_record_lands_in_both_files,
an_ai_only_record_never_reaches_the_human_file, ndjson_lines_parse_independently,
a_file_past_the_size_cap_is_truncated_before_the_next_write, enqueue_without_spawn_does_not_panic}`.

**[Corrigido, 2026-09-24]** `audience` era `"human"` fixo em todo registro (`build_record`), então o
filtro `ai` do painel nunca encontrava nada — vestígio do contrato TS, não um tier funcionando. Agora
`log_buffer.rs::audience_for` deriva o campo do próprio nível: `debug`/`trace` (detalhe de máquina, ver
"Níveis, com critério" abaixo) vira `"ai"`; `info`/`warn`/`error` (o que a pessoa se importa, decide ou
precisa saber) vira `"human"`. Com os dois arquivos reconstruídos nesta rodada (ver acima), o MUST
"tier humano entra TAMBÉM no arquivo de IA" volta a ter objeto de verdade — não só os dois filtros do
painel, os dois ARQUIVOS selecionam subconjuntos reais do mesmo `LogRecord`.

**MUST**: o tier humano é escrito como frase completa em português, com o **efeito** explícito
("caí para a voz local porque a chave do ElevenLabs foi recusada"), não como evento cru
("tts fallback"). API: `logger.human?.(...)`.

**MUST NOT** duplicar o log de dev no tier humano. Uma linha por evento que a pessoa precisa saber —
se o arquivo humano não puder ser lido do começo ao fim, o tier perdeu a razão de existir.

**MUST**: o tier humano entra TAMBÉM no arquivo de IA. Dois arquivos contando metades diferentes da
mesma história é pior que um.

**[Portado, 2026-09-24]** `logger.human?.(...)` virou `human_log!` (`modules/system/human_log.rs`,
`#[macro_export]`, chamado como `crate::human_log!(log::Level::Warn, "...")`). É macro, não função —
precisa expandir no local da chamada para `log::log!` continuar usando `module_path!()` do módulo que
decidiu como `target`/`scope`, não deste arquivo. A macro não muda `audience` (já deriva do nível,
acima); o valor dela é marcar, de forma greppável, os poucos call sites que specs/10-observability.md
nomeia como decisão/sintoma — convertidos para frase-com-efeito explícito nesta rodada:
- `brain/turn.rs::run_turn_llm` — failover pra Ollama: "caí para o Ollama local porque o provedor
  {provider} falhou antes de qualquer efeito colateral (...)".
- `chat/send.rs::handle_chat_send` — provedor de chat desativado: "parei de tentar
  {provider}:{model} no chat porque desativou depois de falhas repetidas (...)".
- `vision/watcher.rs::suspend_for_provider_health` (travado/recuperado) e `run_tick` (desativado) —
  "parei/voltei a observar a tela porque o provedor {provider}:{model} travou/se recuperou/desativou
  (...)".
- `feature_health.rs::record_registration_failure` — boot degradado: "o recurso '{feature}' não vai
  funcionar nesta sessão porque falhou ao registrar (...)".
- `voice/speak_tool.rs::run_speak` — mudo de fala: "fiquei calada porque o mudo de fala ainda está
  valendo" (não havia log NENHUM aqui antes; "a Mika devia falar e ficou calada" não tinha explicação
  no log em lugar nenhum).
- `shortcuts.rs::register_mute_shortcut` — atalho Ctrl+Shift+S recusado pelo SO: "o atalho ... não vai
  funcionar porque o sistema recusou o registro (...)".

Fallback de TTS (ElevenLabs -> Supertonic) existe (`voice/tts/dispatch.rs`, publica
`EventKey::TtsFallback`); a linha humana da troca é responsabilidade de quem assina esse evento.

**MUST**: o console mostra tier humano + `warn`/`error`. `info`/`debug` do tier de IA vivem no
arquivo (`MIKA_LOG_CONSOLE=ai` traz tudo de volta ao terminal).

**[Premissa corrigida, 2026-09-24]** `MIKA_LOG_CONSOLE` não existe em nenhum arquivo do repositório
(grep vazio) — isso está certo. O que a versão anterior desta spec errava: "`system.logLevel` filtra
o que o PAINEL mostra, não o terminal". Falso — conferi `log_level.rs::spawn`, que chama
`log::set_max_level(...)` a partir de `system.logLevel`. `log::set_max_level` é o filtro GLOBAL do
crate `log`, avaliado pelas macros (`log::debug!` etc.) ANTES de qualquer target existir — afeta
stderr, o arquivo `mika-shell` e o ring buffer ao mesmo tempo, não só o painel. Ou seja, o escape
hatch do MUST já existe hoje, só que com outro nome e outro escopo: subir `system.logLevel` para
`debug` no painel de Configurações traz `debug`/`trace` de volta a TODOS os sinks, console incluso —
o mesmo efeito que `MIKA_LOG_CONSOLE=ai` tinha, sem precisar de uma variável de ambiente própria.
Nada a portar aqui: o controle pedido pelo MUST já existe, coberto por `log_level.rs`.

**SHOULD**: toda linha carrega `traceId` do turno corrente (`setLogTrace`), para o log de IA
reconstruir um turno inteiro sem depender de proximidade de horário.

**[Portado, 2026-09-24]** `setLogTrace` (Node) virou `modules::system::log_trace`
(`apps/desktop/src-tauri/src/modules/system/log_trace.rs`): um `tokio::task_local!` (não
`thread_local!` — a `Future` de um turno pode ser retomada em outra worker thread do runtime do
Tauri a cada `.await`, e um `thread_local!` ficaria em branco na próxima retomada). `LogRecord` ganhou
`trace_id: Option<String>` (serializado como `traceId`, batendo com `LogItem`), preenchido em
`log_buffer.rs::build_record` via `log_trace::current()`. O hook fica em
`chat/send.rs::handle_chat_send`: `log_trace::scoped(log_trace::new_id(), turn::run_turn_llm(...)).await`
envolve só a execução do turno — todo `log::` emitido por `brain/turn.rs`, `llm/*` ou uma tool
enquanto essa future estiver rodando carrega o mesmo id, e o painel de Logs já tem `traceFilter`
pronto para filtrar por ele (`logs-panel-filters.ts`, antes morto por falta de produtor).

## Níveis, com critério

| Nível   | Uso                             | Exemplo                                                                   |
| ------- | ------------------------------- | ------------------------------------------------------------------------- |
| `debug` | detalhe de máquina              | chamada IPC, skip por debounce, cache hit, resolução de modelo            |
| `info`  | evento que o usuário se importa | screenshot enviado ao LLM, resposta recebida com tokens, serviço iniciado |
| `warn`  | anomalia recuperável            | retry, fallback, entrada rejeitada                                        |
| `error` | falha que aborta a operação     | provedor desativado, exceção não tratada                                  |

**MUST**: qualquer linha emitida **por iteração de loop** é `debug`. Sem exceção.

> **Por quê:** o loop de captura emitia 9 linhas `info` por ciclo. Em 3 minutos o terminal tinha
> ~170 linhas/minuto e o erro real estava enterrado. Depois de rebaixar as linhas de loop e ligar
> o circuit breaker: ~10 linhas/minuto em 76 minutos de uso.

**[Estado real]** O MUST se mantém no shell Rust: toda linha por-ciclo do screen watcher
(`apps/desktop/src-tauri/src/modules/vision/watcher.rs`) usa `log::debug!` — conferi as ~12 chamadas
do arquivo, nenhuma é `info!`/`warn!` por iteração normal (`warn!` só aparece em falha de catálogo de
pessoas/memória episódica, não por ciclo). O circuit breaker citado no "Por quê", porém, não existe
mais para visão (ver [03](03-resilience.md)) — a lição de nível continua válida, a de circuit breaker
não.

## Formato

**MUST**: `msg` é curto, minúsculo, e descreve o **evento**. Dados estruturados vão em `meta`, com
chaves consistentes.

**MUST NOT** interpolar payload dentro da mensagem. `contentChars: 3400` serve; despejar o prompt
inteiro não.

**MUST**: escopo por módulo, via logger filho. **MUST NOT** usar `console.*` direto — ele fura o
filtro de nível e o painel de Logs da app.

**[Estado real]** `escopo por módulo` sobrevive como `target`/`scope`: `log::warn!` no shell usa o
módulo Rust como target por padrão (ex. `apps/desktop/src-tauri/src/modules/brain/turn.rs:501`), e
`log_buffer.rs::build_record` grava esse target em `scope`. "Logger filho" não existe como API própria
— é o comportamento padrão do crate `log`. `console.*` continua proibido no que resta de TypeScript
(`check:log-discipline`, `scripts/checks/log-discipline.mts`). **[Verificado, 2026-09-24]** Rodei
`npx tsx scripts/checks/log-discipline.mts` isolado: `✓ check:log-discipline — 52 arquivo(s)
conferido(s)`, saída 0. O gate NÃO está rodando vazio — `walk()` (`scripts/checks/lib.mts`) engole a
exceção de `readdirSync` num diretório inexistente e devolve `[]` para essa raiz (`apps/desktop/src/main`),
mas `apps/desktop/src/features` (52 arquivos) e os `packages/*` continuam sendo varridos normalmente.
Piso de alcance real, mas parcial: o gate perdeu silenciosamente a cobertura da raiz que sumiu, sem
nunca ter ficado 100% vazio — vale remover `apps/desktop/src/main` da lista de raízes em
`log-discipline.mts` num commit de limpeza (raiz morta não é mais "piso de alcance", é ruído).

## Segredos

**MUST NOT** logar chave de API, token, senha — nem prefixo, nem tamanho.

**MUST**: presença é booleano: `apiKeyPresent: true`.

> **Por quê:** os primeiros caracteres já identificam a conta em vários provedores, e log vai para
> arquivo e para relatório de erro.

**[Estado real]** O MUST NOT continua cumprido, por um mecanismo diferente do documentado: em vez de
uma convenção de código (nunca logar a chave, reportar só `apiKeyPresent: boolean`), o shell Rust
REDIGE por regex qualquer coisa que pareça segredo ANTES de entrar no ring buffer
(`apps/desktop/src-tauri/src/modules/system/log_redact.rs`: padrões para `sk-`/`gh*_`/JWT, cabeçalho
`Authorization`, `Bearer <token>`, e `chave=valor`/`token=valor`/`key=valor`/`secret=valor`/`password=valor`
— tudo vira `[KEY]`/`[JWT]`). Rede de segurança mais ampla (pega vazamento acidental de string
literal). **[Verificado, 2026-09-24]** `grep -rn "apiKeyPresent" apps/desktop/src-tauri packages` não
acha nada — nenhum campo Rust usa essa convenção explícita hoje; a redação por regex é o único
mecanismo, não um complemento a ela.

## Palavras do usuário

**MUST**: o que o usuário disse/digitou e o que a Mika ouviu/falou entram na linha por inteiro
(`texto`/`text`), junto da FORMA (`textChars`, `contentChars`, `messageCount`) — decisão explícita do
usuário em 2026-09-04, para validar o que a Mika está ouvindo. Revoga a regra anterior (MUST NOT logar
conteúdo, vigente até 2026-09-03) e o gate correspondente (`check:log-discipline` não barra mais
conteúdo de fala).

> **Risco aceito, ciente:** `logs/mika-ai.ndjson` vai junto em qualquer relatório de erro e o painel de
> Logs é tela do app — quem tiver acesso ao arquivo ou à tela lê a conversa completa. Uso local,
> single-user; sem isso a regra anterior existia porque `contentPreview: content.slice(0, 100)` vazou
> no `chat-service` (corrigido em 2026-08-28) — mesma classe de exposição, agora intencional.

**[Ainda não portado, premissa atualizada em 2026-09-24]** `logs/mika-ai.ndjson` voltou a existir
nesta rodada (`modules/system/log_files.rs`, ver "Dois públicos, dois tiers" acima), então o "risco
aceito" acima volta a ter um arquivo concreto de novo — não muda o que é logado, só onde o eventual
conteúdo cairia se alguém o logasse. As rotas de chat em Rust
(`apps/desktop/src-tauri/src/modules/chat/send.rs`, `apps/desktop/src-tauri/src/modules/brain/turn.rs`)
continuam quase sem `log::` de conteúdo — as chamadas que existem hoje nas duas (o failover pra Ollama
e o `disabled` de saúde, ambas convertidas pra `human_log!` nesta rodada, ver "Dois públicos, dois
tiers") logam METADADO de decisão (provedor, motivo), nunca o texto do usuário ou da resposta. A
decisão de 2026-09-04 (logar texto por inteiro) segue sem equivalente no chat Rust: não é que ela foi
revogada, é que a via que a implementava (`chat-service.ts` do Node) foi apagada e o substituto Rust
não loga conteúdo de turno nenhum, nem "por inteiro" nem por forma (`textChars`/`contentChars`) — e
com o arquivo de novo existindo, portar aquilo teria efeito real de novo (arquivo em disco, não só
buffer de memória), então a decisão de NÃO portar (política de conteúdo, fora do escopo de
infraestrutura desta rodada) passa a ter mais peso, não menos.

## Repetição

**MUST**: erro que se repete por causa estrutural loga **na transição**, não por ocorrência.

**SHOULD**: linhas idênticas consecutivas são agregadas com contador em vez de repetidas.

**[Verificado, 2026-09-24]** MUST cumprido: `vision/watcher.rs::update_provider_health` loga a
transição de saúde do provedor UMA vez (`log::info!` só ao travar/recuperar), não por ciclo.

**[Implementado, 2026-09-24]** SHOULD cumprido na camada de sink, que é onde toda fonte converge
(`vision/watcher.rs` incluída, sem precisar tocar cada chamador): `log_buffer.rs::LogRingBuffer::push`
compara a linha nova (nível + escopo + `msg` já redigido) com a última do ring via `same_line`; uma
repetição incrementa `repeat_count` na última entrada do ring E do lote pendente (em vez de duplicar
nos dois) e acrescenta `(xN)` ao `formatted` exibido. `LogRecord` ganhou o campo `repeatCount`
(serializado só quando `> 1`; `logs: z.array(z.unknown())` em `desktop-ipc.ts` aceita o campo extra
sem quebrar o contrato). Cobertura:
`modules::system::log_buffer::tests::{identical_consecutive_lines_collapse_with_a_counter,
a_different_line_breaks_the_repeat_sequence, repeat_counter_also_updates_in_the_pending_batch}`.
**[Painel, 2026-09-28]** `logs-panel-filters.ts::appendLogs` dobra o contador atualizado na linha
já exibida (quando o lote pendente já tinha sido drenado, o backend reenvia a linha com o
`repeatCount` novo — antes ela aparecia duplicada) e `LogLine` mostra o badge `×N`.

## Persistência

**SHOULD**: existe sink de arquivo com rotação, em diretório de dados do usuário — um arquivo por
tier (`logs/mika-human.log`, `logs/mika-ai.ndjson`), mesma rotação e retenção.

**[Cumprido, 2026-09-24]** O par de arquivos existe de novo (`modules/system/log_files.rs`, ver "Dois
públicos, dois tiers" acima), em `<dados-do-app>/logs/`, resolvido por `state::app_data_dir`
(`MIKA_PROFILE_DIR` desviável, como todo outro sink deste processo). Rotação é a mesma nos dois:
truncagem para vazio ao passar de 10 MiB — não a rotação por N arquivos que `tauri-plugin-log` faria
sozinho, mas cobre o MUST "caber em algumas centenas de linhas por sessão" sem histórico acumulado
sem fim. **Nuance não coberta:** `mika-ai.ndjson` recebe o registro CRU (um evento, uma linha, de
propósito — é o tier que quer TUDO); `mika-human.log` recebe o `formatted` do MESMO registro cru, não
a versão colapsada por repetição que o ring buffer aplica pro painel ("Repetição" acima) — um erro
estrutural repetindo rápido pode encher `mika-human.log` de linhas idênticas sem o `(xN)` que o painel
mostra. O SHOULD de "Repetição" foi implementado na camada do ring/painel, não nos arquivos; unificar
os dois exigiria o sink de arquivo também rastrear "última linha por tier", fora do escopo mínimo
desta rodada (o MUST duro, "erro que se repete loga na transição", continua cumprido em
`vision/watcher.rs`, então o pior caso é redundância no arquivo, não perda de sinal).

> **Por quê:** sem arquivo, diagnosticar depende de o usuário ter o terminal aberto e o scrollback
> intacto. Buffer em memória não sobrevive ao fechamento.

**MUST**: o painel de Logs da app lê o mesmo fluxo, com filtro por nível e escopo.

**[Estado real, feature adicionada]** Confirmado e mais completo que o documentado: o painel
(`apps/desktop/src/renderer/src/components/settings/LogsPanel.tsx` +
`use-log-stream.ts`/`logs-panel-filters.ts`) lê `system:getRecentLogs` na carga inicial e um stream de
lote via `onLog` (`log_stream.rs`), com filtro por nível (`system.logLevel`), audiência, busca de
texto, `traceFilter` (não populado hoje, ver acima), pausa, auto-scroll e exportação para `.txt` —
funcionalidade não documentada nesta spec até agora. Commit relevante: `0b9697c4 feat(system): stream
real logs to the settings log panel`.

## Saúde

**MUST**: o app expõe saúde real de boot ([01](01-architecture.md)) e de provedores
([03](03-resilience.md)).

**MUST**: saúde de provedor é evento, não só consulta — a UI reage sem polling.

**[Estado real]** Cumprido para chat: `llm:getProviderHealth` (consulta,
`apps/desktop/src-tauri/src/modules/llm.rs`) + evento `llm:provider-health` emitido a cada
sucesso/falha (`apps/desktop/src-tauri/src/modules/brain/turn.rs`,
`apps/desktop/src-tauri/src/modules/llm/provider_health.rs`) — `ProviderHealthBanner.tsx` reage ao
evento, sem polling. Não cumprido para visão (ver [03](03-resilience.md), "Consumidores param de
trabalhar") — não existe saúde de provedor de visão hoje.

**Não existe monitor central de checagens periódicas, e isso é decisão registrada, não pendência
esquecida.** O `HealthMonitor` do SDK existiu por meses com zero registrantes — a mesma forma que
[03](03-resilience.md) descreve como "controle que existe no papel e lê como seguro em review" — e
foi removido em 2026-08-28. Se a saúde precisar virar LIVENESS de verdade (o banco responde, o host
de inferência está vivo), ela volta **com os registrantes no mesmo commit**: monitor sem check
registrado é pior que nenhum, porque responde "saudável".

## Custo

**MUST**: uso de token e custo estimado por turno são registrados e visíveis.

**SHOULD**: dá para ver quanto cada superfície consumiu (chat, visão, embeddings, resumo). Visão é
a mais cara e a mais fácil de esquecer ligada.

**[Atualizado nesta rodada]** `apps/desktop/src-tauri/src/modules/token_usage.rs` grava o TOTAL
AGREGADO de sempre (`token_usage`, `id = 1`) e também uma quebra por granularidade
(`token_usage_by_provider`, chave `(surface, provider, model)`, migração aditiva em
`telemetry.rs::create_token_usage_table` — não mexe na tabela existente, reversível com um único
`DROP TABLE token_usage_by_provider`). `record()` tem os parâmetros `surface`/`provider`/`model`, e
TRÊS superfícies chamam hoje: `chat` (`chat::send::handle_chat_send`, `chat::proactive::send_proactive`
— provider/model pelo mesmo `resolve_health_key` que já alimenta `provider_health`), `vision`
(`vision::watcher::run_tick`, provider fixo de visão + modelo resolvido) e, desde 2026-09-24,
`gamer-summary` (`gamer::game_watcher::GameWatcher::spawn_llm_summary` — o único turno de LLM real que
o companheiro de jogo dispara fora do chat; `summarize_with_llm` passou a devolver `rig_core::completion::Usage`
junto do texto para isso, provider sempre `"ollama"` porque `run_turn_ollama_raw` é Ollama-only).

**[2026-09-28]** `embeddings` grava quando o provedor devolve contagem: a forma OpenAI
(OpenRouter e endpoint OpenAI-compatível) traz `usage.prompt_tokens`, e
`memory::episodic::embedding.rs::fetch_openai_shaped_embedding` chama
`token_usage::record_embedding`, que usa o `AppHandle` armado no boot (`token_usage::arm`, em
`modules::register_config_bound_tools`) — o ponto de chamada continua sem um. Ollama e o e5 local não
devolvem contagem e são locais (custo zero); Google `embedContent` não devolve contagem.

**[2026-09-28]** `token_usage::cost_for(app, provider, model, usage)` substitui `cost_for_provider`:
- `ollama`: `0.0`, e turno servido pelo failover local também custa `0.0`.
- `openrouter`: preço por token do catálogo vivo (`llm::fetch_openrouter_models` guarda
  `promptPrice`/`completionPrice` em `token_usage::remember_openrouter_prices`). Modelo sem preço
  conhecido conta como desconhecido NESTE turno e dispara a busca do catálogo em segundo plano (no
  máximo a cada 10 min) — a resposta nunca espera a rede.
- `google`, `openaiCompatible` e `unsloth`: desconhecido (`unknown_cost_turns`). Nenhum deles publica preço
  por API (o Unsloth roda na máquina do usuário, mas o endereço pode ser um túnel/LAN — não presumo custo zero), e tabela estática de preço já apodreceu neste repositório (commit `dd130c1b`).

## Métrica de aceitação

Uma sessão de 1 hora, com tudo funcionando, em nível `info`:

- **MUST** caber em algumas centenas de linhas
- **MUST NOT** conter a mesma mensagem mais de 10 vezes
- **MUST** permitir reconstruir o que a Mika fez e por quê

**[Ainda não verificado numericamente]** Não rodei uma sessão de 1 hora real para medir volume de
linha do shell Rust nesta rodada — as duas primeiras métricas seguem sem confirmação numérica; exigem
uso ao vivo, fora do que `cargo test --lib` consegue provar.

**[Parcialmente corrigido, avançado em 2026-09-24, ainda não fechado]** A terceira ("reconstruir o que
a Mika fez e por quê") estava mais fraca do que antes do porte por TRÊS motivos independentes: falta
de `traceId`, falta de frase-com-efeito nos pontos de decisão e falta de log de conteúdo de turno.

Dos três, dois estão resolvidos nesta rodada e na anterior:
- `traceId` — `log_trace` (ver "Dois públicos, dois tiers" acima) dá ao painel de Logs um jeito de
  isolar todas as linhas de UM turno específico (`traceFilter`, antes morto por falta de produtor):
  reconstrução de FLUXO (que módulos rodaram, em que ordem, com que erro).
- **frase-com-efeito nos pontos de decisão** (nesta rodada, `human_log!` — ver "Dois públicos, dois
  tiers") — os seis call sites convertidos (failover, provedor desativado x2, boot degradado, mudo de
  fala x2) agora respondem "POR QUÊ" em português, não só "o quê" (`ProviderBeforeSideEffects`,
  `disabled`, etc. crus). Isso é avanço real sobre "por quê", mas é um subconjunto pequeno e nomeado
  de decisões — a maioria dos `log::` do shell continua terse/técnica por design (tier `ai`).

O terceiro CONTINUA sem objeto, e não foi tocado nesta rodada por decisão de escopo, não por
esquecimento: as rotas de chat em Rust ainda não logam o texto do turno (nem por inteiro, nem por
forma), então reconstruir O QUE foi dito continua dependendo do histórico de chat
(`chat_db`/`history_store.rs`), não do log. Fechar isso exigiria portar a política de "Palavras do
usuário" (revogação de 2026-09-04) para o Rust, o
que este agente não fez por não ser log de INFRAESTRUTURA (rede, config, boot, turno) e sim uma
mudança de política de conteúdo, fora do escopo desta rodada.
