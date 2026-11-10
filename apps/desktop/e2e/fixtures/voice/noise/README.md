# Ruído × fala — fixtures da detecção de voz

Gerados por `pnpm vad:fixtures` (`scripts/generators/generate-vad-fixtures.mts`), PCM16 mono 16 kHz.
Determinísticos: semente fixa, a mesma execução produz os mesmos bytes.

O gabarito é `vad-cases.json`: por arquivo, `expectSpeech` e os trechos de fala (`speechSegments`,
segundos). Um detector bom diz **não** em todo arquivo sem fala e cobre os trechos dos que têm.

| Arquivo | Fala? | O que prova |
|---|---|---|
| `keyboard-mechanical.wav` | não | clique mecânico (~8 teclas/s) não abre a fala |
| `keyboard-membrane.wav` | não | teclado de membrana, mais grave e abafado |
| `keyboard-close-mic.wav` | não | teclado colado no microfone, muito alto — o volume sozinho não decide |
| `mouse-clicks.wav` | não | cliques e duplo-cliques |
| `fan-room-tone.wav` | não | ruído estacionário de ventoinha |
| `keyboard-and-fan.wav` | não | digitação sobre ventoinha |
| `speech-with-typing.wav` | 2,2–6,5 s | fala real com digitação por baixo — ainda é fala |
| `typing-then-command.wav` | 4,2–6,7 s | 4 s só digitando; a fala não pode "começar" antes |
| `quiet-speech-with-fan.wav` | 1,7–6,0 s | fala baixa (-12 dB) sobre ventoinha — ainda é fala |
| `command-with-mouse.wav` | 2,2–4,7 s | comando com cliques de mouse |
| `silence.wav` | não | silêncio digital |

A digitação é sintética de propósito: um clique é um transiente de ataque instantâneo, decaimento de
10–30 ms, banda larga e sem harmônicos — o oposto da voz (f0 80–400 Hz, formantes 300–3400 Hz). As
falas são as já versionadas em `../fala-usuario-pt-br.wav` (humana) e `../commands/abrir-site.wav` (TTS).
