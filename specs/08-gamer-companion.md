# 08 — Gamer companion

## A promessa

Ela joga **com** você. Sabe qual é o jogo, entende o que está acontecendo, comemora, zoa, e sabe
a hora de calar a boca.

## Detecção de jogo

Backend em Rust, `apps/desktop/src-tauri/src/modules/gamer/` (módulo núcleo, não
`modules/features/gamer/` — não existe essa pasta). Duas fontes, complementares:

- **Integração de plataforma** (Steam, via `steam_library.rs`) — jogo, `appId`, biblioteca lida do
  disco (`libraryfolders.vdf`, sem chave de API).
- **Janela em foco + processos em execução** (`game_detection.rs::find_foreground_game`/
  `find_active_game`) — cobre jogo fora da Steam, com uma lista de processos "claramente não-jogo"
  (IDE, browser, launcher de SO) para não confundir título de aba com jogo.

**MUST**: as duas fontes convergem para **um** estado de "jogo atual" —
`game_watcher_decision.rs::decide_transition`, chamado por `game_watcher.rs::GameWatcher::tick`
(janela em foco tem precedência exclusiva sobre processo no mesmo tick, mesma ordem do TS
original). Dois estados divergentes fazem ela comentar do jogo errado.

**MUST**: detecção de jogo dispara evento — `EventKey::GameChanged` (cobre tanto "iniciou" quanto
"trocou", com `initial`/`source` no payload) e `EventKey::GameStopped`, publicados por
`GameWatcher::apply`. Não há um terceiro evento distinto para "trocou" — é o mesmo `GameChanged` de
início, o que hoje não diferencia os dois casos no nome do evento (só no payload).

## Estado de comportamento

**MUST**: existe um estado explícito de "gaming" que muda o comportamento do sistema inteiro:
intervalo de captura de tela diferente, proatividade reduzida, respostas mais curtas.

**Portado parcialmente em 2026-09-24** —
`apps/desktop/src-tauri/src/modules/gamer/gaming_state.rs::GamingState` é o estado explícito único
(um `AtomicBool` de processo, `Arc` injetado, mesmo desenho de
`brain::awareness::tension::GameTensionState`): atualizado por `EventKey::GameChanged`/`GameStopped`
no assinante de `brain::awareness::spawn_proactive_engine`, exposto por `is_active()` para quem mais
precisar ler (ainda sem chamador de visão — ver abaixo).

- **Proatividade reduzida para candidatos fora do jogo**: portado —
  `ProactiveEngine::should_speak` (`brain/awareness/engine.rs`) recebe um `is_gaming: Fn() -> bool` e,
  quando `true` e o candidato NÃO é de jogo (`gaming_state::is_game_candidate_key`, prefixo `"game"`:
  `game:{game}`/`game-sustained`/`game-absence`), reduz a proatividade efetiva pela metade antes de
  calcular o cooldown (`gates::cooldown_for`) — o jogo em si mantém proatividade cheia. Testes:
  `gaming_state_stretches_the_cooldown_for_a_non_game_candidate` /
  `_does_not_stretch_the_cooldown_for_a_game_candidate`.
- **Intervalo de captura de tela diferente**: **portado em 2026-09-24** —
  `vision::watcher::active_interval_ms` lê `GamingState::is_active()` e divide o intervalo por 6,
  com piso próprio de jogo (`GAME_MIN_INTERVAL_MS`, 15s). Ver [06](06-vision.md).
- **Respostas mais curtas**: **portado em 2026-09-24** para o chat geral —
  `brain::persona::gaming_context::build_gaming_context_block` (o MESMO bloco pull-based que já lê
  `SessionMemory::current_session()` para "## Sessão de Gaming Atual") agora acrescenta uma instrução
  de brevidade quando há sessão aberta ("responda em 1-2 frases curtas..."). Instrução de PROMPT, não
  fala pronta (AGENTS.md §2.6) — sem parâmetro novo em `run_turn_ollama` (que tem vários chamadores
  fora do gamer). Teste:
  `build_gaming_context_block_reports_the_active_game_and_a_positive_duration` (asserção nova).
  Não confundir com o comentário reativo de momento (`game_moment_commentary.rs`), que já é curto por
  template de prompt, não por este mecanismo.

**MUST**: entrar e sair do estado reinicia os timers dependentes. Trocar de estado sem reagendar
deixa o intervalo antigo rodando.

**Portado em 2026-09-24** para a cadência de proatividade —
`ProactiveEngine::on_gaming_transition` limpa o cooldown POR CHAVE de todo candidato; chamado no
mesmo assinante de `GameChanged`/`GameStopped` só quando `GamingState::set_active` reporta uma
transição REAL (ociosa->jogando ou jogando->ociosa, nunca uma troca de jogo no meio de uma sessão já
ativa). Teste: `on_gaming_transition_clears_the_per_key_cooldown_so_a_stale_timer_does_not_linger`.
O intervalo de captura de tela (fora do escopo, ver acima) continua sem esse reinício.

## Silêncio é feature

**MUST NOT** falar durante momento de tensão — combate, boss, clutch, cutscene. Portado:
`apps/desktop/src-tauri/src/modules/brain/awareness/tension.rs::GameTensionState` — `boss`/
`cutscene` abrem tensão (90s/45s de janela, configurável por `gamer-companion.momentCooldowns`),
`death`/`victory`/`defeat` fecham. A MESMA instância é consultada por `ProactiveEngine`,
`vision::watcher` (suprime o comentário de tela) e `gamer::game_moment_commentary` (suprime o
comentário de backseat gaming) — os três concordam sobre a janela.

**MUST**: existe um jeito de silenciar na hora, por atalho ou por voz, sem abrir Settings e sem
esperar um turno de LLM.

- **Por atalho** — Ctrl+Shift+S (`apps/desktop/src-tauri/src/shortcuts.rs::register_mute_shortcut`),
  que alterna `EventKey::MuteSpeech` consumido por `voice::tts::speech_mute`.
- **Por voz — portado em 2026-09-24** (`modules/voice/silence_command.rs::handle`, canal
  `mika:voice-silence-command`). `apps/desktop/src/renderer/src/hooks/useShellVoice.ts::onTranscript`
  — o ÚNICO funil por onde todo transcript FINAL passa antes de virar turno, tanto do reconhecimento
  do navegador quanto do fallback `MediaRecorder`/Whisper (mesmo `onTranscript`, ver o cabeçalho de
  `useSpeechRecognition.ts`) — chama este canal ANTES da decisão busy/hold
  (`decideSpokenTurn`/`handleSpokenTurn`), inclusive com um turno em curso. Um comando reconhecido
  (`"cala a boca"`, `"silêncio"`, `"shut up"`, `"cállate"`, `"tais-toi"`, entre outros — pt-BR/en/es/fr,
  casamento conservador: frase inteira ou comando abrindo a fala, NUNCA substring no meio, ver os
  testes de `silence_command.rs`) publica `EventKey::MuteSpeech` direto no barramento, com a MESMA
  janela do atalho (`crate::shortcuts::MUTE_WINDOW_MS`, 15 min) — e a fala NUNCA chega a
  `sendMessage`/o LLM. Se a Mika já estava falando, o renderer também para a reprodução local
  (`stopTTS()`, mesmo caminho que uma fala nova qualquer já usava). **SHOULD** desmutar por voz,
  simétrico (`"pode falar"`/`"you can talk"`/etc.) — feito.

**SHOULD**: ela aprende os momentos em que foi mandada calar e antecipa.

**Portado em 2026-09-24** —
`apps/desktop/src-tauri/src/modules/gamer/mute_learning.rs::MuteLearning`. Reusa o MESMO mecanismo de
silêncio do atalho em vez de inventar um segundo: `observe_moment` (chamado a cada
`EventKey::GameMoment`, síncrono e puro — testável sem `bus()`) credita um tipo de momento quando um
`MuteSpeech{muted:true}` REAL (não o antecipado por ela mesma, marcado com `"source":
"mute_learning"`) chega dentro de 20s de um momento daquele tipo; depois de 2 créditos, o PRÓXIMO
momento do mesmo tipo já chega com `anticipate_mute` publicando `MuteSpeech` de antemão, antes de
`game_moment_commentary` decidir se comenta. `game_learning.rs` continua sendo outra coisa (fatos do
jogo via wiki), não este aprendizado.

> Uma companion que fala na hora errada é pior que nenhuma. O custo de um comentário perdido é
> zero; o custo de um comentário no meio de um clutch é o usuário desligar a feature.

## Momentos

Ela reage a eventos do jogo: morte boba, vitória apertada, conquista (`death`/`victory`/`defeat`/
`achievement`, `COMMENTED_TYPES` em
`apps/desktop/src-tauri/src/modules/gamer/game_moment_commentary.rs`). "Sessão longa demais" é outro
provider proativo (`GameProvider`/`gaming-loop`), não um `momentType` de comentário reativo.

**MUST**: todo momento tem cooldown próprio e configurável — piso duro de 15s entre QUALQUER
comentário + cooldown por tipo (`gamer-companion.momentCooldowns[type]`, com default de 60s para
`achievement`/`victory` e 45s para `death`/`defeat`), tudo em `game_moment_commentary.rs`
(`CommentaryState::is_blocked`). Sem isso ela vira alarme. **Comentário de backseat gaming vem
desligado por padrão** (`gamer-companion.backseatGamingEnabled = false`).

**MUST**: reação a momento é **gerada por LLM** com o contexto do momento
(`CommentGenerator::generate` -> `turn::run_turn_llm_raw`). Nunca frase pronta.
Ver [07](07-persona-and-relationship.md).

**MUST**: momento vira episódio na memória ([04](04-memory.md)) —
`apps/desktop/src-tauri/src/modules/memory/experience.rs::store_game_moment`, chamado no mesmo
assinante de `EventKey::GameMoment` que aplica a tensão
(`apps/desktop/src-tauri/src/modules/brain/awareness/mod.rs`). "Lembra daquela vez que você morreu
pro primeiro mob?" é o retorno do investimento.

## Ajuda no jogo

**SHOULD**: com visão ligada, ela ajuda com o que está na tela — item, build, puzzle, erro. Isso é
o chat comum consumindo o contexto de tela que a visão já publica ([06](06-vision.md)), não uma
feature separada de "ajuda" — **Verificado**: `brain::persona::screen_context` assina
`EventKey::ScreenObservation`, guarda o resumo em prosa mais recente e injeta um bloco "## O Que
Está na Tela Agora" no `system.md` de todo turno de chat regular
(`brain::persona::compose_system_prompt`), atrás de três guardas — `vision.enabled`,
`privacy::is_screen_capture_allowed` e frescor (TTL = `min(2× vision.captureIntervalMs, 4min)`).
Sem mecanismo dedicado de "ajuda" além disso: o LLM responde com o que está no prompt, no tom da
persona.

**MUST**: ajuda é oferecida no tom da persona, não como painel de dicas. Consistente com "toda fala
da Mika é gerada por LLM" (AGENTS.md §2.6) — não há template de dica no código do gamer companion.

**MUST NOT** dar dica não pedida em jogo competitivo durante a partida. Portado para o comentário
reativo de momento: `game_moment_commentary.rs::is_competitive_game` bloqueia `death`/`achievement`
em jogos competitivos (só `victory`/`defeat` passam o gate de tipo). **Verificado, ausente**: essa
regra NÃO se aplica a uma dica dada em resposta a uma pergunta direta do usuário no chat —
`is_competitive_game` só tem um chamador (`game_moment_commentary.rs`), então uma dica pedida
explicitamente durante uma partida competitiva não passa por gate nenhum. A frase "não pedida" do
MUST já cobre esse caso à parte (pergunta direta é pedido, por definição), mas o texto original da
spec estava ambíguo sobre se a regra também deveria valer aqui — respondido: não, por design.

## Sessão

**MUST**: sessão de jogo tem início, fim e resumo —
`apps/desktop/src-tauri/src/modules/gamer/session_memory.rs::SessionMemory` (`start_session`/
`end_session`, persistida em SQLite via `session_db.rs`). **Portado em 2026-09-24**: o resumo agora
vira episódio de longo prazo — `game_watcher.rs::GameWatcher::spawn_llm_summary` chama
`memory::experience::store_game_session_summary(game_name, summary)` depois que
`summarize_with_llm` já tentou, com o resumo FINAL da sessão (o do LLM quando deu certo, senão o
`fallback_summary` que `end_session` já tinha persistido — nunca as duas entradas). Núcleo puro
testável sem runtime: `game_watcher.rs::final_session_summary(llm_result, fallback)`. A tool
`game_session` (`describe`) continua existindo à parte, sob demanda do próprio agente — o novo
caminho é automático, no fim de toda sessão. Testes: `final_session_summary_prefers_a_non_empty_
llm_result`/`_falls_back_when_the_llm_result_is_empty`/`_falls_back_when_the_llm_call_failed`
(`game_watcher.rs`), `store_game_session_summary_with_an_empty_game_or_summary_is_a_no_op`
(`memory/experience.rs`).

**MUST — cumprido em 2026-09-24**: o resumo agora É gerado por LLM, com `fallback_summary`
(`session_types.rs`) só como fallback. `session_memory.rs::end_session` continua síncrono e
persiste o fallback NA HORA (nunca bloqueia esperando o LLM); `GameWatcher::apply`
(`game_watcher.rs`) captura a sessão recém-encerrada e `GameWatcher::tick` dispara
`session_memory::summarize_with_llm` (`turn::run_turn_ollama_raw`, mesmo caminho que
`game_session`/`game_knowledge` já usavam) em segundo plano
(`tauri::async_runtime::spawn`, fora de `apply()` para continuar testável sem runtime); se o LLM
responder com texto não-vazio, `SessionMemory::set_summary` substitui o fallback no SQLite. Uma falha
do LLM deixa o fallback determinístico valendo. Testes:
`summarize_with_llm_fails_named_without_touching_a_real_provider_when_unreachable` (porta morta,
nunca provedor pago/rede real), `set_summary_overwrites_a_persisted_summary`,
`apply_returns_the_ended_session_when_the_game_stops`/`_when_switching_games_without_an_intermediate_stop`.

**SHOULD — cumprido em 2026-09-24**: "ela puxa a sessão anterior ao ver o mesmo jogo abrir de novo".
`GameWatcher::apply` consulta `SessionMemory::session_for_game(&game_id)` ANTES de abrir a nova
sessão do mesmo jogo e publica o resumo encontrado como `previousSessionSummary` no payload de
`EventKey::GameChanged`; `brain::awareness::event_reactions::on_game_changed` injeta esse resumo no
`{{ctx}}` do template `apps/desktop/prompts/brain/awareness/steam-game.md` (antes sempre vazio), então a reação proativa "jogo
começou" já menciona a sessão anterior quando existe. Testes:
`apply_publishes_the_previous_session_summary_when_the_same_game_reopens` (`game_watcher.rs`),
`game_changed_mentions_the_previous_session_summary_when_present` (`event_reactions.rs`).

## Pessoas no jogo

**SHOULD — portado em 2026-09-24**: pessoas vistas na tela (inclusive parceiros de jogo) entram no
catálogo via auto-upsert de visão
(`apps/desktop/src-tauri/src/modules/vision/watcher.rs::upsert_detected_people`,
`MentionSource::Vision`), e agora a relação "joga com" existe: `RelationshipType::PlaysWith`
(`apps/desktop/src-tauri/src/modules/features/person_catalog/types.rs`), travada como
`Creator`/`Vvip` em `merge_fields::resolve_relationship_type` (não regride por afinidade). Atribuída
por `apps/desktop/src-tauri/src/modules/gamer/game_partners.rs::promote_recurring_partner` —
assinante de `EventKey::PersonMentionAdded` (`spawn_partner_watcher`, ligado em `modules/mod.rs`)
que promove a pessoa quando ela foi mencionada 3+ vezes COM sessão de jogo aberta no momento de cada
menção. **Fechado em 2026-09-24**: a rodada anterior usava `PersonRecord::mention_count` (o total,
qualquer fonte — chat, voz, visão) como piso, aproximação nomeada no próprio código. Agora
`game_partners::PartnerMentionTracker` conta, por pessoa, só as menções creditadas com sessão aberta
— uma menção no chat sobre alguém fora de sessão nunca soma para "joga com" (contador em memória por
processo, reinicia com o app, mesmo padrão de `gaming_state::GamingState`). Testes:
`promotes_a_recurring_partner_seen_while_a_game_session_is_open`,
`does_not_promote_below_the_recurrence_threshold`, `does_not_promote_without_an_open_game_session`,
`mentions_made_without_a_session_never_count_toward_the_threshold` (`game_partners.rs`),
`resolve_relationship_type_locks_plays_with_regardless_of_affinity`
(`merge_fields.rs`). Sem UI dedicada, por convenção já existente: `relationship_type`
(`PersonRecord.relationship_type`, a classificação automática por afinidade) já não tinha superfície
nenhuma de UI para NENHUMA das 9 variantes anteriores — o campo é declarado em
`person-catalog.types.ts` mas não é lido por nenhuma view do renderer; a lista renderizada em
`RelationshipRow` (`person-detail-parts.tsx`) é OUTRO campo (`PersonRecord.relationships[].type`,
texto livre, o grafo social da pessoa com outros nomes, não a relação dela com a Mika). `plays_with`
segue a mesma convenção do campo que estende: automático, sem controle de edição, mesma ausência de
UI que `creator`/`vvip`/o resto da escada já tinham. `RelationshipSelector.tsx` é uma feature
DIFERENTE (a relação da própria Mika com o usuário), não tocada nesta rodada.

**MUST**: valem as mesmas regras de privacidade — são terceiros. Confirmado: o auto-upsert de
`watcher.rs` exclui confiança `low` e nunca grava nome vazio, mesma regra que o resto do catálogo.
