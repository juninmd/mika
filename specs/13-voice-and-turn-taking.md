# 13 — Voz e turno de fala

## Ouvir e falar são o mesmo laço

Não são dois recursos (STT de um lado, TTS do outro). São um único laço de turno: o momento em que
a Mika para de ouvir é o momento em que ela decide o que falar, e o momento em que o usuário volta a
falar é o momento em que ela **para**. Um bug em qualquer ponta do laço aparece como "ela não presta
atenção" ou "ela fala por cima de mim" — o sintoma número um de "isso é um bot", não uma amiga.

## STT

**MUST**: o motor de STT padrão é local — Nemotron 3.5 ASR Streaming 0.6B (int4 ONNX), escolhido por
alucinar menos em ruído que Whisper, com Whisper como _fallback_ local configurável.

**MUST**: a escolha de modelo é um controle **vivo** — mudar o seletor muda o modelo que roda.

> **Por quê:** o seletor "Modelo Whisper (Local)" (tiny/base/small, rotulado "Small — melhor em
> pt-BR") é **morto**. `apps/desktop/src-tauri/src/modules/desktop.rs` (`WHISPER_FILES`) e
> `apps/desktop/src-tauri/src/modules/inference/whisper.rs` baixam e carregam `whisper-base` sempre,
> qualquer que seja a escolha — o valor de `voice.whisperModel` só é ecoado de volta na tela de
> teste de conexão (`modules/settings/plugin_test.rs`), nunca lido pelo caminho de download ou de
> inferência. O usuário troca a configuração, a UI confirma, e nada muda.

**Estado em 2026-09-26**: Nemotron é o motor real, default, e transcreve pt-BR corretamente (testado
e confirmado em log de produção); Whisper como alternativa local está preso no checkpoint fraco
(`whisper-base`) pelo defeito acima, e nenhum dos dois é o estado da arte em pt-BR (Nemotron
~5,5% WER no FLEURS, atrás de Whisper large-v3 ~3,65% e Canary-1B-v2 ~4,39% — números do próprio
fabricante, não medidos aqui).

**MUST**: não existe pontuação de confiança do STT hoje em nenhum dos dois motores nem na nuvem —
`is_implausible_transcript` é heurística (densidade de caracteres, frases-clichê), não confiança
real do modelo. **SHOULD**: expor a confiança quando o motor a fornecer (Nemotron RNNT expõe score
de joiner) e usar como sinal adicional, não só a heurística.

## Contrato de áudio

**MUST**: o backend aceita exclusivamente PCM16 mono 16 kHz. Decodificação de qualquer outro formato
de áudio é responsabilidade do renderer, nunca do Rust.

## Pré-roll (não perder o começo da fala)

**MUST**: o buffer de captura mantém uma janela anterior ao gatilho de fala (pré-roll), para que o
começo da frase não seja cortado pela latência de detecção de voz.

> **Por quê:** medido em 2026-09-26 (smoke com voz real injetada): **toda** utterance perdeu as
> primeiras palavras — "Oi Mika, tudo bem?" virou "Tudo bem", "Para, para!" perdeu o "Para,". A
> captura só começa depois que o gate de amplitude cruza o limiar; sem pré-roll, esse atraso de
> detecção é onset irrecuperável, não erro de STT. Hoje não existe pré-roll em nenhuma camada
> (`speech-transcribe.ts`/`useSpeechRecognition.media.ts`).

## Fim de turno (end-of-turn)

**Estado em 2026-09-26**: fim de turno é puramente acústico e hoje tem **dois** mecanismos
independentes, e é fácil confundir um pelo outro:

1. **Escuta normal** — `SILENCE_HANGOVER_MS`, uma constante hardcoded de **1200ms**
   (`apps/desktop/src/renderer/src/hooks/speech-transcribe.ts:43`), consumida por
   `useSpeechRecognition.media.ts:174`. Silêncio de amplitude por esse tanto encerra a utterance e
   dispara `mika:voice-transcribe`.
2. **Barge-in / interrupção** — `InterruptionDetector::check_silence_endpoint`
   (`apps/desktop/src-tauri/src/modules/voice/interruption_detector.rs`), só ativo quando `self.speaking`
   é verdadeiro (Mika está falando), com `silence_ms` configurável (default 1500ms,
   `voice.vadSilenceMs`).

**MUST**: todo controle de UI que promete mudar o comportamento de escuta muda o comportamento de
escuta.

> **Por quê:** "Duração de Silêncio para Finalizar" (`voice.vadSilenceMs`/`voice.vadEndpointingEnabled`,
> `apps/desktop/src-tauri/src/config/schemas/voice.rs:99`) só alimenta o mecanismo 2 (barge-in), que só
> roda enquanto Mika fala. Para a escuta normal — o caso comum, frase do usuário sem Mika estar
> falando — o corte é sempre o hardcoded 1200ms do mecanismo 1, e o controle na tela não tem efeito
> nenhum sobre ele. Mesma classe de defeito do seletor de Whisper acima: controle vivo na tela,
> morto no runtime.

**SHOULD (meta)**: fim de turno híbrido — o VAD acústico continua como piso de segurança (nunca
espera para sempre), mas um classificador leve de fim-de-frase semântico adia o corte quando a
frase parece incompleta (pergunta em aberto, conjunção pendente). Referência concreta:
`pipecat-ai/smart-turn-v3` — modelo ONNX de 8M parâmetros sobre encoder Whisper-Tiny, BSD-2-Clause,
~12ms de inferência em CPU, com português listado — não integrado hoje.

## Barge-in

**Estado em 2026-09-26**: o mecanismo central é real e rápido — VAD Silero detecta fala em ~96ms (3
janelas de 32ms, `trigger_streak: 3`, `NEURAL_SPEECH_THRESHOLD: 0.5`) e dispara dois efeitos: parar o
áudio da Mika (`ForceStopTts`) e abortar o turno de LLM em voo (`chat::turns::abort_turn`, um
`tokio::select!` que cancela o stream de rede de verdade, não só uma flag olhada entre chunks).

**MUST**: fala do usuário interrompe **tanto** a reprodução de TTS **quanto** o turno de LLM em voo.
Parar só o áudio e deixar o turno antigo terminar em segundo plano é a mesma classe de bug que
"resposta chega depois que ninguém pediu mais".

> **Por quê:** hoje isso só é verdade para o caminho de VAD. Interrupção por **texto** ou por
> ditado-contínuo (o usuário termina de falar/digitar enquanto Mika ainda fala) para o áudio local
> (`stopTTS`) mas **nunca** cancela o turno de backend em voo — o turno antigo segue rodando e pode
> empurrar uma resposta obsoleta para o histórico/UI depois do fato.

**MUST**: uma interrupção produz **exatamente uma** resposta.

> **Por quê:** o caminho rápido de VAD (`awareness::on_interruption` → `send_proactive`,
> `apps/desktop/src-tauri/src/modules/chat/proactive.rs:59`) e o reconhecimento contínuo do renderer
> (`useShellVoice.ts`, que termina de transcrever a MESMA utterance no seu próprio timer de
> endpointing e chama o `mika:chat-send` normal) podem os dois responder à mesma interrupção — corrida
> real, não teórica, com apenas uma guarda de obsolescência unidirecional entre os dois.

**MUST**: toda resposta em voo — incluindo a resposta-à-interrupção — é abortável por um novo
barge-in.

> **Por quê:** `send_proactive` cria seu próprio `TurnAbortHandle` sem registrá-lo em
> `chat_turns::register_turn`. Um segundo barge-in durante essa resposta cala o áudio
> (`ForceStopTts` ainda funciona, via geração incrementada no `auto_speak`) mas
> `chat_turns::abort_turn` não encontra nada registrado para essa janela — o turno de LLM da
> resposta-à-interrupção continua rodando sem ninguém ouvindo.

**MUST NOT** falar proativamente enquanto o usuário está falando.

> **Por quê:** medido em 2026-09-26 — comentários proativos sobre a tela ("esse parkour de gato...")
> foram falados **enquanto o usuário estava no meio de uma frase**, sem checar o estado de escuta.

**MUST**: a voz da própria Mika **MUST NOT** disparar barge-in nela mesma (autoeco).

**SHOULD (meta)**: hoje a rejeição de autoeco é só **pós-hoc** — checada depois da transcrição
(`speak_guard::looks_like_self_echo`), não antes do `ForceStopTts` disparar. Num setup de caixa de
som sem AEC perfeito, a própria fala da Mika pode cruzar o limiar de probabilidade e cortá-la no meio
da frase. Meta: sinal preventivo em `feed_vad_sample`/`set_playback` — limiar mais alto (ou streak
maior) durante uma janela de graça logo após o início da reprodução.

**MUST**: não existe cobertura E2E nem fixture de áudio adversarial (tosse, teclado, áudio de jogo)
para este fluxo, apesar do histórico de bugs multi-commit. Este é um gap declarado, não um MUST
cumprido — ver [11](11-quality-gates.md) para o padrão de probe negativo que qualquer gate novo aqui
precisa.

## Falar enquanto o LLM ainda gera

**MUST**: a síntese de voz começa a partir do primeiro trecho falável do stream, não espera o LLM
terminar. Portado — `brain::turn` emite `TextChunk` por token, `voice::auto_speak` corta em sentenças
por um algoritmo real de mín/máx caracteres e enfileira para um worker de TTS serial.

**MUST**: instrumentação de tempo-até-primeiro-áudio (time-to-first-audio) existe para provar que o
design segura na prática.

> **Por quê:** sem número medido, "a primeira frase sai antes do LLM terminar?" era inferência de
> código. Hoje `voice/auto_speak/turn_timing.rs` mede o tempo até o primeiro áudio a partir do mesmo
> instante (`ThinkingStart`) do "primeiro token" de `brain/turn.rs`, e `spawn_speak_worker` loga.

## O que nunca é falado

**MUST NOT** falar (nem guardar em memória como se fosse fala real) o raciocínio narrado do modelo —
"[thinking] O usuário quer..., vou responder...".

> **Por quê:** medido em 2026-09-26 — a maioria das respostas começava com
> `<|THINKING|> [thinking] O usuário perguntou sobre o jogo... Vou responder de forma natural...`, e
> esse trecho **ia para o TTS e era gravado na memória** como se fosse fala real.
> `brain/turn/narration_filter.rs` corta esse parágrafo no stream, antes de `TurnEvent::TextChunk`
> (um filtro novo por passo do turno), e preserva um "pensando alto" que fala COM o usuário. Prova de
> que está ligado, não só compilado: `turn/tests.rs::narration_filter_is_wired_so_leaked_reasoning_never_reaches_text_chunk_events_or_the_final_text`.

**MUST NOT** ler bloco de código em voz alta.

**MUST NOT** ler URL crua em voz alta — um resumo curto substitui o link.

> **Por quê:** antes, `sanitize_text` (`voice/tts/preprocessor.rs`) lia os símbolos de um bloco
> ` ``` ` caractere a caractere, e um link `[texto](url)` perdia as duas partes. Hoje o bloco cercado
> inteiro vira uma deixa falada e o link Markdown fica só com o texto visível (regexes `FENCE` e
> `LINK` do mesmo arquivo).

## Fala de preenchimento durante ferramenta

**SHOULD (meta)**: quando o modelo chama uma ferramenta antes de ter gerado qualquer texto falável,
uma frase curta de preenchimento ("deixa eu ver...") é falada, gated por latência mínima estimada da
ferramenta.

> **Por quê:** hoje não existe. `handle_tool_call`
> (`apps/desktop/src-tauri/src/modules/voice/auto_speak/mod.rs:167`) só libera o que já estava no buffer
> — vazio quando a ferramenta é a primeira coisa do turno — e o usuário fica em silêncio total pelo
> tempo da chamada (segundos, em rede/ferramenta lenta).

## Orçamento de latência

Números de 2026-09-26, LLM local (Gemma 4 26B via llama-server, prompt de sistema ~10,6k
tokens/20,8k caracteres), voz injetada por microfone falso (WAVs SAPI pt-BR), medidos no trace de
eventos do renderer e em sonda direta contra o `llama-server`.

| Etapa                                          | Hoje (medido)                                              | Meta                                                                 |
| ---------------------------------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------- |
| Fim da fala → chamada de STT                   | 2,3–2,5 s (hangover fixo 1200ms + decodificação)           | ≤ 400 ms — pré-roll cobre o onset; hangover deixa de ser fixo        |
| STT (Nemotron)                                 | 2,8–8,3 s para 4,7–12,8 s de áudio (RTF 0,6–0,9 sob carga) | RTF < 0,3 sustentado                                                 |
| LLM — primeiro token, cache de prompt **hit**  | 0,8–2 s                                                    | mantém                                                               |
| LLM — primeiro token, cache de prompt **miss** | 8–11 s                                                     | da ordem do hit — ver "Pinagem de slot" em [14](14-agent-harness.md) |
| Primeiro áudio de TTS                          | sem instrumentação (não medido)                            | instrumentado, p95 documentado                                       |
| Detecção de barge-in                           | ~96 ms (3 janelas Silero)                                  | mantém                                                               |
| Parada de reprodução no barge-in               | não medida                                                 | < 150 ms ponta a ponta, medido                                       |

**MUST**: quando este orçamento for instrumentado, os números entram como `Verificado`, nunca como
suposição — mesma régra de [11](11-quality-gates.md) ("a prova é quantitativa").

## Quem está falando (SHOULD/MAY, visão de futuro)

**SHOULD**: só a voz do dono (usuário principal) governa o turno — voz de terceiro (TV, jogo com
chat de voz, outra pessoa no cômodo) não deve abrir nem fechar um turno de conversa.

**Estado em 2026-09-26**: não existe. Zero ocorrência de identificação de locutor/diarização/
embedding de voz em todo o backend — qualquer voz captada acima do limiar vira "o usuário", sem
distinção.

**MAY**: um perfil de voz do usuário principal, opt-in e consentido, comparado por embedding leve
sobre a mesma janela de VAD já embarcada (Silero/Whisper), sem enviar áudio para fora da máquina.

## Prosódia e emoção do usuário (SHOULD, visão de futuro)

**SHOULD**: o tom de voz do usuário (irritado, animado, cansado) é um sinal disponível para o turno,
não só o texto que o STT devolveu.

**Estado em 2026-09-26**: não existe. [06](06-vision.md) e [07](07-persona-and-relationship.md)
cobrem a emoção **da Mika**; nada analisa pitch/energia/taxa de fala do áudio do usuário. Meta: um
classificador leve sobre o mesmo áudio que já passa pelo VAD, alimentando um campo opcional no
contexto do turno — mesmo padrão de custo baixo que a visão já usa (heurística antes de LLM).

## Wake word (MAY, visão de futuro)

**MAY**: ativação por palavra-chave ("Mika, ...") como alternativa ao toggle manual de microfone
(F9/Ctrl+Space).

**Estado em 2026-09-26**: não existe — zero ocorrência de wake-word/hotword em código, i18n ou
specs. Não crítico dado que voz já é entrada de primeira classe com microfone sempre ligável
([00](00-vision-and-scope.md)), mas evitaria apertar botão a cada turno. Se implementado: detector
leve rodando antes do gate de amplitude, opt-in, nunca substituindo o toggle manual existente.
