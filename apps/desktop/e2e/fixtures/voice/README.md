# Fixture de voz — homologação do STT

Dois arquivos, a MESMA gravação: 6,1 s de fala real em português, capturada do microfone do usuário
na sessão de 2026-08-23.

- `fala-usuario-pt-br.webm` — o original, no formato que o **renderer** produz
  (`audio/webm;codecs=opus`). É o que o MediaRecorder entrega.
- `fala-usuario-pt-br.wav` — PCM16 mono a 16 kHz, o formato que o **backend Rust** recebe. Derivado do
  webm uma única vez (`ffmpeg -ac 1 -ar 16000 -c:a pcm_s16le`); regenerar só se o original mudar.

Os dois existem porque a decodificação mora no renderer (Chromium/WebView2 tem `decodeAudioData`); o
backend só aceita PCM16 mono 16 kHz (AGENTS.md §2.5). Quem consome o WAV hoje são os testes Rust:
`inference/vad.rs`, `voice/vad_bridge.rs`, `voice/vad_pipeline.rs` (VAD/barge-in) e os `#[ignore]` de
`inference/whisper.rs` e `inference/nemotron/mod.rs` (precisam dos pesos baixados). Nenhum spec E2E
nem script do repositório consome o `.webm`; ele é o registro do formato de origem.

## Por que uma gravação real, e não um sintético

O STT foi homologado até aqui olhando log: `audio transcribed {"durationMs":9147,"textChars":11}`.
Isso diz que **algo** foi transcrito, não que foi transcrito **certo** — e a diferença apareceu na
primeira vez que alguém leu o texto: o trecho do meio saiu `"Apatiferente ela"`, que não é português.
Contagem de caracteres não pega isso. Áudio de verdade, com transcrição conferida, pega.

O log não guarda o texto de propósito (disciplina de log: conteúdo do usuário não vai para log), então
o fixture é a única forma de ter a fala e a transcrição lado a lado.

## Transcrição de referência

Medida com `faster-whisper` modelo `base`, device CPU, `language=pt`:

```
[idioma pt · prob 1.00 · duração 6.1s]
 0.00s → 1.84s   "Ah, ela voltou!"
 1.84s → 3.96s   "Apatiferente ela, eu estou mudando ela."   ← corrompido pelo modelo `base`
 3.96s → 4.76s   "Ah..."
```

O segundo segmento está **errado de propósito no registro**: é o resultado real do modelo `base`, e
serve de linha de base. Se um dia alguém subir o modelo (`small`, `medium`) ou trocar o provedor, é
esse trecho que deve melhorar — e é por ele que se mede a melhora.

## Como usar

Os testes Rust que dependem só do WAV rodam na suíte padrão (`pnpm test:rust`). A transcrição de
referência acima só é conferida pelos testes `#[ignore]` (pesos ONNX + tokenizer baixados pelo app):

```
cargo test --lib -p mika-desktop -- --ignored transcribe_matches_known_reference_wav
MIKA_NEMOTRON_DIR=<pasta com nemotron-cache/> cargo test --lib -p mika-desktop -- --ignored nemotron
```

O antigo `pnpm homologate:voice` (Whisper via transformers.js, sidecar Node) e a extensão Python que
ele resolvia saíram da árvore; a stack é TypeScript + Rust, com o gate `check:no-python` segurando a
porta.
