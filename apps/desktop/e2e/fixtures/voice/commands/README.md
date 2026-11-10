# Comandos de voz — fixtures

Um `.wav` por comando falado, gerados por `pnpm homologate:voice-commands` com o TTS local
(Supertonic voz `F3`, `speed` 0.85, 44,1 kHz) e consumidos pelo mesmo runtime Whisper do app.

## Por que ficam versionados

O script só sintetiza o que **não existe**. Um fixture que se regenera a cada rodada não é fixture:
a síntese do Supertonic é difusão com ruído, e a mesma frase, na mesma voz e velocidade, sai
diferente a cada geração. Medido aqui: *"Mika, quanto tempo eu joguei hoje?"* saiu limpo numa rodada
e como *"quanto eu joguei hoje?"* na seguinte. Com os arquivos fixos, uma piora na transcrição é uma
piora na ESCUTA, e não a loteria da síntese.

Para regravar de propósito, apague o `.wav` e rode o script de novo.

## O que eles não são

Não são voz humana. São o mais perto que dá para chegar sem gravação, e servem para provar que o
caminho `áudio → PCM16 16 kHz → Whisper → texto` está de pé e que o pedido do usuário chega
reconhecível ao modelo. A homologação com fala humana real é `pnpm homologate:voice`, contra
`../fala-usuario-pt-br.wav`.

## Números medidos (2026-08-28, whisper-base, esta máquina)

| Modelo | RTF | Comandos íntegros |
|---|---|---|
| `whisper-base` | 0,33 | 5/8 |
| `whisper-small` | 1,14 | 6/8 |

`small` acerta um pouco mais e custa **3,5x** mais tempo — com RTF acima de 1 a transcrição demora
mais que a própria fala, o que fura a prioridade de latência do repo. Por isso o default segue
`base` (`voice.whisperModel` em `apps/desktop/src-tauri/src/config/schemas/voice.rs`).
