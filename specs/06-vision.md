# 06 — Visão

## O que a visão entrega

A Mika sabe o que está na sua tela **agora**, sem você contar. É o que faz a diferença entre
"conversa com uma IA" e "alguém está do seu lado".

## Pipeline

Backend em Rust — `apps/desktop/src-tauri/src/modules/vision/watcher.rs::run_tick`:

```
tick (intervalo adaptativo, ver abaixo) → gate de observação (should_tick) → provedor saudável?
     → captura na qualidade configurada (thumbnailQuality fixo) → hash da própria captura → mudou?
     → debounce (recaptura até estabilizar ou esgotar as rodadas) → recompressão adaptativa
     → gate de densidade de texto → seleção de imagens por analysisMode → LLM de visão
     → separação comentário/GAME_SIGNAL → gameAwareness → EventBus + diário visual + token_usage
```

**[Removido no porte Tauri]** Não existe mais uma "captura barata (probe)" separada antes da
captura completa: o backend Rust captura UMA vez, na qualidade de `vision.captureQuality`
(`apps/desktop/src-tauri/src/modules/vision/screen_capture.rs::capture_screen_frames`), e calcula
o dHash de mudança (`image_hash.rs::compute_images_hash`) sobre essa MESMA captura — não há um
segundo disparo em qualidade menor para decidir se vale a pena capturar de novo em qualidade alta.

**MUST**: cada etapa antes do LLM existe para **não** chamar o LLM. Chamada de visão é o item mais
caro do produto — imagem em base64 por ciclo.

## Gates obrigatórios antes de gastar

Ordem real (`watcher.rs::run_tick`):

1. **Feature ligada** — `vision.enabled` + `vision.visionAssistActive` + (`vision.autoDescribeScreen`
   OU sorteio de `vision.proactiveCommentaryChance`) (`should_tick`).
   **[Portado nesta rodada]** "provedor saudável": logo após `should_tick` e ANTES da captura,
   `run_tick` chama `suspend_for_provider_health` (`watcher.rs`), que consulta
   `provider_health::is_blocked("ollama", vision_model)` — ver a nota abaixo e [03](03-resilience.md).
2. **Captura** — `screen_capture.rs::capture_screen_frames`, já gateada por
   `privacy::is_screen_capture_allowed` e pelas duas blocklists (janela em foco e janela visível,
   `capture_gate.rs`).
3. **Hash** — `image_hash.rs::has_screen_changed`: tela igual à anterior (dHash, limiar de 10%) não
   é analisada.
4. **Densidade de texto** — quando `vision.codeAwareness` está ligado, `text_density.rs` barra a
   análise se nenhuma captura tem UI/texto suficiente (`vision.textDensityThreshold`).

**[Bug corrigido nesta rodada]** Os gates 3 e 4 acima decodificam a captura via
`crate::images::resize_bitmap`, que por sua vez chamava `decode_data_url` — e essa função exigia uma
data URL completa (`data:image/...;base64,`). `screen_capture.rs::capture_screen_frames` sempre
devolveu base64 CRU, sem esse prefixo (o mesmo formato que já ia direto para o LLM via
`UserContent::image_base64`, sem passar por `images.rs`). Resultado: toda captura de tela REAL
falhava a decodificar nos dois gates — o hash saía sempre vazio (fail-open de `has_screen_changed`:
dedup nunca disparava, cada tick era "sempre mudou") e a densidade de texto saía sempre `0.0`
(fail-closed: com `codeAwareness` ligado, a análise era barrada SEMPRE). Os testes de
`image_hash.rs`/`text_density.rs` nunca pegaram isso porque só usavam data URLs completas
(fixture sintética), nunca base64 cru como uma captura real produz. `images.rs::decode_data_url`
agora aceita os dois formatos — teste de regressão em `src/images.rs` (`resize_bitmap_decodes_raw_
base64_without_a_data_url_prefix`/`encode_jpeg_decodes_raw_base64_without_a_data_url_prefix`).

**[Portado nesta rodada]** Debounce e intervalo adaptativo por jogo em foco.

- **Debounce** (`watcher.rs::run_tick`, após o hash detectar mudança): recaptura em janelas de
  `DEBOUNCE_WINDOW_MS` (1,5s) até duas capturas SUCESSIVAS baterem
  (`is_debounce_settled`, que reusa `has_screen_changed`) ou esgotar `DEBOUNCE_MAX_ROUNDS` (3) — a
  análise roda sobre a captura ESTÁVEL, não sobre o primeiro frame de uma transição (menu abrindo,
  cutscene). Rajada de mudanças vira UMA análise só, sobre o estado final, não uma por frame
  intermediário.
- **Intervalo adaptativo** — `watcher.rs::active_interval_ms`, chamada em `spawn_screen_watcher` a
  cada volta do laço: com `GamingState::is_active()` verdadeiro, o intervalo configurado é dividido
  por 6 e clampado num piso PRÓPRIO de jogo (`GAME_MIN_INTERVAL_MS`, 15s — mais baixo que o piso
  geral de 60s, porque o estado de um jogo muda rápido demais para esperar um minuto entre
  observações). `GamingState` é a MESMA instância de processo que `brain::awareness::
  spawn_proactive_engine` atualiza a partir de `GameChanged`/`GameStopped` (`modules/mod.rs`).

**[Corrigido]** Dois furos do laço periódico (`watcher.rs`):

- **Primeira sonda.** Com a sonda de janela em foco suportada mas sem nenhum snapshot ainda, o gate
  de foco (`capture_gate::is_foreground_blocked`) é fail-open: um app da blocklist podia ir ao
  modelo no primeiro tick. `periodic_capture_ready` pula o tick até existir o primeiro snapshot
  (a captura sob demanda segue valendo).
- **Hash só após a análise.** `capture_changed_screen` gravava o hash antes do LLM; uma falha do
  provedor "consumia" a tela e ela só era tentada de novo quando mudasse. Agora `hash_after_tick`
  só deixa de gravar quando o LLM falha (`TickOutcome::Failed`); análise feita ou gate de densidade
  gravam.

O diário visual passa por `redact_pii` antes de ir à memória, como os `newFacts`.

**MUST**: pular por qualquer gate loga em `debug`, nunca `info`. Confirmado em `run_tick` (todo
`return` cedo usa `log::debug!`).

## Custo

**MUST**: existe teto de tamanho por imagem, e a qualidade se ajusta sozinha para respeitá-lo.

**MUST**: a compressão é adaptativa nas duas direções — sobe qualidade quando as capturas estão
bem abaixo do alvo.

**[Portado nesta rodada]** os dois MUSTs acima, em `vision/compression.rs`
(`compress_images_for_llm`/`next_quality`), chamado por `watcher.rs::run_tick` logo depois do
debounce e ANTES do gate de densidade de texto (as duas etapas leem a MESMA representação que vai
para o LLM):

- `vision.llmQuality`/`vision.maxImageDimension` SEMPRE se aplicam (reencode via
  `crate::images::encode_jpeg`, existente mas sem chamador real até esta rodada) — o teto de
  dimensão não depende do toggle adaptativo.
- `vision.adaptiveCompression` liga o ajuste fino por tamanho de arquivo: acima de
  `vision.maxImageSizeKB`, a qualidade desce; bem abaixo (< metade) de
  `vision.compressionTargetSizeKB`, a qualidade sobe — `next_quality` é a decisão pura, bounded a 4
  idas por imagem (`MAX_ROUNDS`), nunca converge para sempre.
- `vision.analysisMode` (`watcher.rs::select_images_for_mode`) — `"focused"` manda só a primeira
  imagem capturada (multi-monitor), `"balanced"` até duas, `"full"` todas. Espelha a descrição da UI
  (`focused`: "Analyzes only the main element"; `full`: "Deep analysis of the entire screen").

As seis chaves (`adaptiveCompression`/`maxImageSizeKB`/`compressionTargetSizeKB`/`llmQuality`/
`maxImageDimension`/`analysisMode`) agora têm leitor real; nenhuma é mais um toggle sem efeito.

**[Portado nesta rodada]** Custo de token registrado: `llm_client::analyze_with_usage` expõe o
`rig_core::completion::Usage` da resposta, e `watcher.rs::run_tick` grava em `token_usage::record`
com `surface="vision"` quando o provedor de fato reportou uso (`usage_reported`, `Usage::default()`
— todos os campos zero — significa "provedor não informou", nunca "turno grátis", e nunca é
gravado). Antes desta rodada `specs/10-observability.md` já documentava que só o chat gravava —
visão/embeddings/resumo continuavam invisíveis no painel de custo apesar de gastarem imagem em
base64 por ciclo. Os outros chamadores de `analyze` (`vision_capture`/`vision_read`/clipboard) não
têm `AppHandle` disponível (`ToolHandler::call` não recebe um — ver `mod.rs` §3) e continuam sem
gravar; `analyze` (a fachada usada por eles) descarta o uso, `analyze_with_usage` (só o watcher, que
TEM `AppHandle`) o expõe.

**MUST**: quando o provedor de visão está travado, o watcher **para de capturar**. Ver
[03](03-resilience.md).

**[Portado nesta rodada]** `watcher.rs::run_tick` chama `suspend_for_provider_health` ANTES da
captura (não só antes do LLM) — um par `("ollama", vision_model)` bloqueado (`provider_health::
is_blocked`) faz o tick inteiro retornar cedo, sem gastar CPU/disco com a captura. O próprio
`run_tick` também alimenta esse registro: `record_success` em toda análise bem-sucedida,
`record_failure` (com status extraído do erro) em toda `VisionLlmError::Provider`. A transição para
suspenso/retomado loga uma vez (`log::info!`/`log::warn!`), nunca por ciclo — ver
[03](03-resilience.md) para a classificação fatal/transiente e o latch pré-I/O que agora cobrem
visão E chat com o MESMO `provider_health.rs`.

> **Por quê:** com um modelo inválido, o watcher (então em TypeScript) subia ~140 KB por ciclo para
> receber HTTP 400. O gasto era invisível porque nada na UI indicava falha. Esta rodada portou a
> defesa que fechou esse incidente para o lado Rust.

## Modelo de visão

**MUST**: o modelo de visão é configurável **independente** do modelo de chat. O modelo de chat
pode ser texto-só; forçar o mesmo quebra a visão.

**MUST**: a seleção é por catálogo, filtrada por capacidade de visão quando o provedor informa,
com escape quando não informa. Ver [02](02-providers.md).

**MUST NOT** aceitar id de modelo digitado à mão.

> **Por quê:** o campo era texto livre. Uma tag do Ollama foi salva com OpenRouter selecionado, e
> nada — nem UI, nem schema, nem factory — reclamou até virar 400 em loop.

## Saída

A análise produz uma observação **estruturada**, não um parágrafo solto:

- descrição do que está na tela
- app em foco (título, processo)
- é jogo? qual?
- tem código? erro visível?
- é engraçado / meme-worthy?
- entidades reconhecidas

**MUST**: a observação vai para o EventBus e para o cache de contexto de tela, consumido pelo
prompt do próximo turno — **Verificado**: `watcher.rs` publica `EventKey::ScreenObservation`
(`observation_payload`), `brain::persona::screen_context::spawn` assina e guarda `analysis`/`at` num
holder de processo, e `brain::persona::compose_system_prompt` lê esse holder a cada turno de chat
(atrás de `vision.enabled`/`privacy::is_screen_capture_allowed`/frescor — ver [08](08-gamer-
companion.md) §"Ajuda no jogo").

**MUST**: emissão da observação acontece **uma vez** por ciclo, independente de qual etapa a
produziu.

## Controles de percepção

O usuário escolhe o que ela **não** deve comentar:

- reconhecimento de código e erros de compilação (`vision.codeAwareness`) — **lido**, gateia a
  densidade de texto em `watcher.rs::run_tick`.
- reconhecimento de jogos, HUD e menus (`vision.gameAwareness`) — **[Portado nesta rodada]**:
  `watcher.rs::apply_game_awareness`, chamada logo depois de `split_game_signal` (produção e o
  gancho de teste `ingest_analysis_for_e2e`) — desligado, zera `isLikelyGame`/`game`/`gameMoment`/
  `tense` do sinal ANTES de qualquer efeito colateral (fatos, `GameMoment` publicado, checagem de
  tensão, payload). O modelo continua sendo perguntado (o prompt não muda por usuário), mas a Mika
  para de AGIR sobre a resposta — os campos não relacionados a jogo (`isFunny`/`people`/
  `platformContext`/`memeContext`/`newFacts`) não são tocados.
- leitura de imagens da área de transferência (`vision.readClipboardImages`) — **lido**, um dos
  três gates de `clipboard_watch.rs::should_watch_clipboard`.
- diário visual (`vision.visualDiaryEnabled`/`vision.visualDiaryIntervalCycles`) — **[Portado nesta
  rodada]**: `watcher.rs::should_record_diary_entry` + `store_visual_diary_entry`. Um contador de
  ANÁLISES bem-sucedidas (não de ticks — um ciclo só conta quando chega até a resposta do LLM,
  nunca quando é pulado por gate) incrementa a cada `run_tick` bem-sucedido; a cada
  `visualDiaryIntervalCycles` análises, o comentário vira uma entrada de memória episódica
  (`metadata = {type: "visual-diary", source: "vision", game?}`) — igual em espírito a
  `store_discovered_facts`, independente do gate de comentário/tensão abaixo (o que a Mika "viu"
  fica registrado mesmo quando ela não fala sobre). O gancho `ingest_analysis_for_e2e` não tem
  contador persistente entre chamadas (é uma invocação isolada, não um laço) — escopo deliberado,
  o diário só roda no laço real.

**MUST — cumprido para os 4 toggles**: todo toggle da UI tem leitor no main.

## Comportamento proativo

**MUST**: comentário espontâneo sobre a tela é **probabilístico e configurável**, não a cada
observação.

**MUST NOT** comentar durante momento de tensão do jogo. Ver [08](08-gamer-companion.md).

**MUST**: todo comentário é gerado por LLM com o contexto da observação. Nunca template.

## Privacidade

**MUST**: visão é opt-in explícito.

**MUST**: existe lista de apps/janelas que nunca são capturados (gerenciador de senhas, banco).

**MUST NOT** persistir screenshot bruto por padrão. O que fica é a observação em texto.

**MUST NOT** logar o conteúdo da imagem. Só metadados: contagem, tamanho, qualidade.
