# 04 — Memória

## O que "ter memória" significa aqui

Não é histórico de chat. É a Mika saber, sem você repetir, que você odeia jogo competitivo depois
da meia-noite, que terminou Elden Ring mês passado, e que o Bruno é seu amigo do trabalho que
sempre te chama pra Counter-Strike.

## As quatro camadas

| Camada                      | Escopo      | Some quando   | Alimenta                 |
| --------------------------- | ----------- | ------------- | ------------------------ |
| **Trabalho**                | turno atual | fim do turno  | o prompt imediato        |
| **Curto prazo**             | sessão      | app fecha     | continuidade da conversa |
| **Longo prazo — fatos**     | permanente  | usuário apaga | quem você é              |
| **Longo prazo — episódios** | permanente  | usuário apaga | o que aconteceu          |

O catálogo de pessoas é uma quinta camada, especializada: [05](05-person-catalog.md). No porte
Tauri o código do catálogo mora em `apps/desktop/src-tauri/src/modules/features/person_catalog/`
(feature tardia), separado da memória social de fatos/relações
(`apps/desktop/src-tauri/src/modules/memory/social/`) que este documento cobre — os dois módulos
colaboram (a extração automática de menção do catálogo dispara sobre eventos do chat), mas não são
o mesmo diretório.

## Curto prazo

**MUST**: a janela recente entra crua no prompt, limitada por `historyLimit`.

**MUST**: acima do limite, o excedente é **resumido**, não truncado. Perder o começo da conversa
sem avisar faz a Mika parecer desatenta.

**MUST**: o resumo rolante é gerado por LLM e guardado, não recalculado a cada turno.

**Portado em 2026-09-24 (corte e resumo)**: no backend Rust, `mika:chat-send` manda o histórico REAL
de turnos anteriores para o modelo — resolvendo a regressão anterior de sempre mandar
`history: vec![]` (ver o cabeçalho de `apps/desktop/src-tauri/src/modules/chat/mod.rs`).
`HistoryStore::to_turn_history` (`apps/desktop/src-tauri/src/modules/chat/history_store.rs`) recebe
`limit` e devolve só os `limit` turnos MAIS RECENTES (`send.rs` resolve `llm.historyLimit` do
`ConfigStore`, default 20, e passa adiante) — a primeira MUST está portada e testada
(`to_turn_history_keeps_only_the_most_recent_turns_within_the_limit`).

A segunda e a terceira MUST (resumo do excedente, gerado por LLM e guardado) agora têm produtor:
`chat/rolling_summary.rs::summarize_overflow_with_llm`, mesmo desenho de
`gamer::session_memory::summarize_with_llm` — `send::handle_chat_send` grava o par
usuário/assistente do turno (síncrono) e SÓ DEPOIS checa `HistoryStore::overflow_for_summary(limit)`;
quando há mensagens além de `historyLimit` que o resumo ainda não cobre, dispara o resumo em segundo
plano (`tauri::async_runtime::spawn`, nunca no caminho de resposta). O resumo é INCREMENTAL — o
anterior entra no prompt do LLM junto do trecho novo, não é recalculado do zero a cada chamada — e é
persistido por sessão (`chat_rolling_summary` em `chat.db`, `summarized_through` marca até onde já
foi resumido). `to_turn_history` injeta o resumo guardado como mensagem `role: system` ANTES da
janela recente crua (`rig_core::completion::Message::system`, nunca lido como fala real do usuário
ou da assistente). Uma falha do LLM só loga e deixa `summarized_through` parado — a próxima chamada
tenta de novo o mesmo trecho, nunca um resumo fabricado. Testes: `to_turn_history_prepends_the_
rolling_summary_when_present`, `overflow_for_summary_returns_only_the_slice_not_yet_summarized`,
`rolling_summary_survives_restart_when_persisted` (`history_store.rs`),
`summarize_overflow_with_llm_fails_named_without_touching_a_real_provider_when_unreachable`
(`rolling_summary.rs`, porta morta — nunca provedor pago/rede real).

## Longo prazo — fatos

Fatos são afirmações curtas e atemporais sobre o usuário ou o mundo dele.

**MUST**: extração de fatos roda **depois** do turno, fora do caminho de resposta. Nunca atrasa a
fala da Mika. Portado — `post_turn::pipeline::run`
(`apps/desktop/src-tauri/src/modules/brain/post_turn/pipeline.rs`) chama
`post_turn::facts::run` depois de já ter gravado o turno, nunca no caminho de streaming da
resposta.

**MUST**: fato tem origem rastreável (de qual turno saiu) e data.

**MUST**: fato contraditório **atualiza** o anterior em vez de coexistir. "Joga Valorant" e "parou
de jogar Valorant" não podem estar ativos ao mesmo tempo. Portado — `contradicts`
(`apps/desktop/src-tauri/src/modules/memory/social/fact_rules.rs`) detecta negação sobre o mesmo
núcleo semântico e `add_fact`
(`apps/desktop/src-tauri/src/modules/memory/social/fact_store.rs`) marca o fato anterior
`active: false` com `superseded_by` apontando para o novo.

**SHOULD**: fatos têm confiança, e a Mika trata fato de baixa confiança como suposição na fala
("acho que você tinha falado que...").

## Longo prazo — episódios

Episódio é "o que aconteceu", com quando e contexto. É o que permite "lembra daquela vez que...".

**MUST**: busca episódica é semântica (embeddings), com fallback por palavra-chave quando
embeddings estão indisponíveis.

**MUST**: o fallback por palavra-chave **quebra a pergunta em termos**. Mandar a pergunta inteira
como um único `LIKE %...%` retorna vazio para qualquer pergunta real. Portado e testado —
`tokenize_query`/`score_keyword_candidates` em
`apps/desktop/src-tauri/src/modules/memory/episodic/search.rs` — e é o caminho que roda quando
embedding não está configurado, o provider falha, ou (Google/OpenRouter) o modo offline está ligado
(ver "Embeddings" abaixo: os quatro provedores têm cliente Rust real desde 2026-09-24).

> **Por quê:** esse fallback existia, era documentado, e devolvia vazio sempre. Ninguém percebeu
> porque o caminho com embeddings funcionava.

## Embeddings

**MUST**: provedor de embedding é configurável e independente do provedor de chat — usar Gemini
para conversa e Ollama local para embedding é caso de uso legítimo, não gambiarra. A aba
`embeddings` (`llm.embeddingProvider`/`llm.embeddingModel`/`llm.embeddingTaskType`/
`llm.embeddingOutputDimensionality`) é declarada em
`apps/desktop/src-tauri/src/modules/memory/settings.rs`.

**MUST**: os provedores aceitos são os mesmos **cinco** ([02](02-providers.md)) — `google`,
`openrouter`, `ollama`, `openaiCompatible`, `unsloth` (mesma lista em
`apps/desktop/src-tauri/src/modules/memory/settings.rs`).

**[Removido no porte Tauri]**: o provedor embarcado `local` (transformers.js) **saiu** do catálogo,
das settings e do código junto com `packages/llm-local`/`packages/embedding-local` — ver a seção
"Os quatro" de [02](02-providers.md). O **default de fábrica** de `llm.embeddingProvider` passou a ser
`ollama`, não mais `local`; memória semântica sem chave e sem nuvem agora depende do usuário ter um
Ollama rodando (`http://localhost:11434`), não de um runtime ONNX embarcado.

**[Portado nesta rodada: os quatro]** `generate_embedding`
(`apps/desktop/src-tauri/src/modules/memory/episodic/embedding.rs`) chama de verdade o provedor
configurado — `ollama` (`{ollamaBaseUrl}/api/embeddings`), `google`
(`generativelanguage.googleapis.com/v1beta/models/{model}:embedContent`, key por query string),
`openrouter` (`openrouter.ai/api/v1/embeddings`, forma OpenAI), `openaiCompatible`
(`{baseUrl}/embeddings`, mesma forma) e `unsloth` (`{unslothBaseUrl}/embeddings`, mesma forma, mas SEM
modelo padrão: o servidor serve o embedding GGUF carregado e o app não inventa um nome) — via `resolve_embedding_target`, a MESMA tabela que o botão
"Validar" da aba `embeddings` (`settings::plugin_test::test_embedding_connection`) usa, para as duas
nunca divergirem sobre qual provider está ativo. Cada provider resolve a própria credencial
(`apiKeys.googleAI`/`apiKeys.openrouter`/`apiKeys.openaiCompatibleBaseUrl`+`apiKeys.openaiCompatibleApiKey`/
`apiKeys.unslothBaseUrl`+`apiKeys.unslothApiKey`)
— nunca a de outro provider ("par assimétrico endereço↔credencial", mesma classe de defeito já
corrigida em [02](02-providers.md) para o catálogo Ollama); `endpoint_and_key` em `plugin_test.rs`
garante o mesmo do lado da tela.

`EmbeddingFetcher` continua injetável (mesmo padrão de `AchievementsFetcher`,
`gamer/steam_achievements`), agora com um resolvedor por trás (`resolve_embedding_target`) numa
thread dedicada com runtime `current_thread` próprio (os chamadores são síncronos, chamados de
dentro de tarefas que já rodam sobre o runtime Tokio do processo — ver o cabeçalho do arquivo para o
porquê de não ser `async fn`). Passa pelo mesmo chokepoint de privacidade do resto do processo:
`privacy.offlineMode` bloqueia a chamada quando o endereço não é loopback — para Google/OpenRouter
(endereço fixo, nunca loopback) isso bloqueia sempre que o modo offline está ligado, igual às URLs
fixas de `modules/llm.rs`. Um 6º nome de provider (fora dos cinco) continua caindo no fallback por
palavra-chave, honesto, nunca lança. Testes: `resolve_embedding_target_*`/`generate_embedding_via_*`
(`embedding.rs`) e `embeddings_google_without_a_credential_fails_named_not_as_unported`/
`embeddings_unknown_provider_is_named_not_silently_ignored` (`plugin_test_tests.rs`) — sem servidor
real no ar (dublê injetado ou recusa antes de qualquer rede).

**MUST**: trocar de modelo de embedding invalida o índice vetorial. Dimensões diferentes produzem
vizinhos sem sentido, silenciosamente. Portado e testado:
`invalidate_index_if_embedding_changed` em `apps/desktop/src-tauri/src/modules/memory/episodic/embedding.rs`.

**MUST**: o catálogo de modelos de embedding é **separado** do catálogo de chat.

> **Por quê:** os endpoints `/models` retornam modelos de chat, e o do Gemini filtra por
> `generateContent`, que **exclui** modelos de embedding. Reaproveitar o seletor de chat aqui
> listaria a coisa errada com cara de certa.

## Aprendizado contínuo

**MUST**: a Mika aprende **sem cadastro manual**. O usuário nunca é obrigado a preencher formulário
para ela saber algo.

Fluxo: turno → extração assíncrona (fatos, pessoas citadas, emoção, tópicos) → gravação → entra no
contexto do próximo turno.

**MUST**: extração falhando **MUST NOT** derrubar o turno nem aparecer para o usuário. Degrada em
silêncio, loga em `warn`. Portado — `extract_facts`
(`apps/desktop/src-tauri/src/modules/brain/post_turn/facts.rs`) nunca propaga erro de prompt
ausente ou de modelo: cai em lista vazia com `log::warn!`.

**MUST**: o usuário pode ver, editar e apagar tudo que ela aprendeu. Memória que não dá para
corrigir vira mentira permanente.

## Recuperação para o prompt

Cada turno monta contexto com orçamento de tokens, em ordem de prioridade:

1. persona e nível de relacionamento
2. resumo rolante + janela recente
3. fatos do usuário relevantes à mensagem
4. pessoas citadas na mensagem
5. episódios semanticamente próximos
6. contexto de tela ([06](06-vision.md))
7. estado de humor e afinidade

**MUST**: quando o orçamento estoura, corta-se de baixo para cima. Persona e conversa recente são
os últimos a sair.

**MUST**: o que entrou no contexto é inspecionável para diagnóstico. Portado — `TurnTrace::record`
(`apps/desktop/src-tauri/src/modules/brain/turn.rs`) guarda o `systemPrompt`/`userContent` reais do
último turno.

**Portado**: `persona::compose_system_prompt`
(`apps/desktop/src-tauri/src/modules/brain/persona/mod.rs`) agora concatena sete fontes de contexto
dinâmico, na ordem de prioridade desta seção: gíria, memória de SESSÃO, jogo ativo, metas (as quatro
já portadas), e as três que fechavam esta lacuna — fatos de longo prazo
(`build_long_term_facts_block`, via `memory::social_reader().get_graph()`, só fatos ATIVOS, 5 mais
recentes, 140 caracteres cada), pessoas citadas na mensagem
(`build_mentioned_people_block`, via `person_catalog::catalog_reader().mentioned_in`, considera
apelidos, MUST NOT chamar LLM neste caminho síncrono — 3 pessoas, bio a 200 caracteres) e episódios
semânticos (`build_episodic_context_block`, delega a `EpisodicMemoryService::build_context`, que já
faz semântico-com-fallback-por-palavra-chave e aplica `rag_top_k`). `compose_system_prompt` passou a
receber `user_content` (o texto do turno atual) — `brain::turn::run_turn_ollama`/`run_turn_llm`
repassam o que já tinham. Os tetos por fonte (facts/pessoas/episódios) são o corte de orçamento desta
seção: um limite fixo por turno, não contagem de tokens.

## Privacidade

**MUST**: memória é local por padrão.

**MUST**: existe um jeito de apagar tudo, e ele apaga de verdade — incluindo vetores.

**MUST NOT** enviar memória para provedor de LLM fora do contexto do turno que a exige.
