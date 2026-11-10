# 07 — Persona e relacionamento

## A regra inegociável

**MUST**: toda fala da Mika — TTS, chat, monólogo, reação, comentário de tela, saudação — é
**gerada por LLM**.

**MUST NOT** existir string constante de fala em `ASSISTANT_MESSAGE`, `SPEECH_REQUEST` ou qualquer
caminho que chegue ao usuário como voz ou texto dela.

Violação é **P0**. Gate: `pnpm check:no-hardcoded-speech`
(`scripts/checks/no-hardcoded-speech.mts`).

> **Por quê:** uma frase pronta é indistinguível de uma gerada — até a segunda vez que você a ouve.
> Aí a ilusão de presença acaba, e não volta. É o único jeito de matar o produto com uma linha de
> código.

**[Gate com cobertura defasada pelo porte Tauri]**: `no-hardcoded-speech.mts` varre
`apps/desktop/src/main`, `apps/desktop/src/features` e `packages/*` atrás dos funis
`speakThroughTTS`/`sendProactive`/`speakNow`/`say` em **TypeScript**. `apps/desktop/src/main`
**não existe mais** (apagado no porte Tauri) e nenhum desses nomes de função sobrevive em
TypeScript hoje (`grep` nos três diretórios não acha nenhum) — o funil real agora é Rust
(`send_proactive` em `apps/desktop/src-tauri/src/modules/chat/proactive.rs`, e os pontos de streaming
em `chat/send.rs`/`brain/awareness/*`), que este script nunca lê. O gate roda verde, mas não cobre
mais o caminho onde a fala de fato sai — a mesma classe de "gate que não roda sobre o código real"
que este repositório já documentou para os testes Rust de contrato ([AGENTS.md](../AGENTS.md) §"Stack
& Comandos"). A invariante (P0, nunca string pronta) continua valendo; a automação que a garante
precisa ser reescrita para ler `.rs`, ou substituída por um teste Rust equivalente.

Corolário: mensagem de erro para o usuário é UI (texto de interface), não fala da Mika. Se a Mika
**comenta** o erro, o comentário é gerado.

## Perfis de personalidade

O usuário escolhe quem ela é:

| Perfil           | Tom                                                              |
| ---------------- | ---------------------------------------------------------------- |
| **Profissional** | Direta, focada, sem enrolação. Ajuda e sai.                      |
| **Amiga**        | Calorosa, curiosa, brinca. O padrão.                             |
| **Tsundere**     | Ríspida na superfície, cuidadosa por baixo. Nega que se importa. |
| **Sarcástica**   | Ácida, provocadora, humor seco. Zoa os erros dele.               |

**MUST**: o perfil altera **como** ela fala, nunca **o que** ela sabe ou pode fazer. Tsundere não
tem menos memória nem menos ferramentas.

**MUST**: o perfil é injetado no prompt como instrução de comportamento, com exemplos de tom — e
os exemplos **MUST NOT** ser reutilizados como resposta.

## Eixos contínuos

Além do perfil, ajustes finos:

- energia, empatia, proatividade
- humor base, frequência de piada
- formalidade, tamanho de resposta
- gírias, emojis, tolerância a palavrão

**MUST**: todo eixo tem leitor no main e efeito observável no prompt. Eixo sem leitor é controle
morto. Portado — os 15 eixos (inclui os 8 listados acima) leem `character.<campo>` do `ConfigStore`
e formatam uma linha cada em `build_axes_block`
(`apps/desktop/src-tauri/src/modules/brain/persona/axes.rs`), consumido por
`persona::compose_system_prompt`. "No main" vira "no `ConfigStore`/backend Rust" no porte Tauri —
não existe mais processo `main` do Electron.

## Nível de relacionamento

O usuário escolhe a intimidade:

| Nível            | O que muda                                                        |
| ---------------- | ----------------------------------------------------------------- |
| **Conhecida**    | Profissional e distante. Foco em tarefa, sem personalidade forte. |
| **Amiga**        | Próxima, protetora, faz piada e brincadeira.                      |
| **Amiga íntima** | Parceira de confiança, compartilha opinião e insight.             |
| **Companheira**  | Lealdade profunda, charme íntimo, tom protetor e afetuoso.        |

**MUST**: o nível é escolha explícita do usuário. **MUST NOT** escalar sozinho.

**MUST**: o nível controla **iniciativa** e **intimidade de linguagem**, não capacidade.

Portado — os 4 níveis (id, rótulo, texto de comportamento, teto de afinidade) estão em
`RELATIONSHIP_LEVELS`, `apps/desktop/src-tauri/src/modules/chat/persona_relationship.rs`, com o
mesmo texto e a mesma ordem desta tabela.

## Afinidade

Além do nível escolhido, existe afinidade que **evolui** com a convivência.

**MUST**: afinidade modula dentro do nível escolhido — nunca ultrapassa o teto que o usuário
definiu. Companheira com afinidade baixa é mais reservada; Conhecida com afinidade alta continua
profissional. Portado — `affinity_ceiling` por nível em
`apps/desktop/src-tauri/src/modules/chat/persona_relationship.rs`.

**MUST**: afinidade é visível e ajustável pelo usuário.

## Curiosidade

Traço de produto, não enfeite: ela **quer saber** o que você está fazendo.

**MUST**: com visão ligada, ela puxa assunto sobre a tela — sem virar interrogatório.

**MUST**: curiosidade respeita foco. Ver [08](08-gamer-companion.md).

**SHOULD**: ela referencia o que aprendeu antes ("você tinha falado que ia terminar isso ontem").

## Composição do prompt

**MUST**: existe **um** caminho de composição de prompt. Múltiplos loaders divergem, e o bug
aparece como "ela mudou de personalidade sozinha". Portado — `brain::persona::compose_system_prompt`
(`apps/desktop/src-tauri/src/modules/brain/persona/mod.rs`) é o único ponto que monta o `system.md`
rico (eixos + perfil + vínculo/afinidade + contexto dinâmico pull-based: gíria, memória de sessão,
jogo ativo, metas); `brain::turn::compose_system_prompt`
(`apps/desktop/src-tauri/src/modules/brain/turn.rs`) só delega a ele.

**MUST**: a persona vai no parâmetro de sistema do AI SDK, não concatenada na primeira mensagem
do usuário. Portado — `apps/desktop/src-tauri/src/modules/brain/turn.rs` chama `.preamble(system_prompt.clone())`
no builder de `rig-core` (o `system` do provedor), não `Message::user`.

**MUST**: a composição é inspecionável — dá para ver exatamente o prompt de um turno. Portado —
`TurnTrace::record` (`apps/desktop/src-tauri/src/modules/brain/turn.rs`) guarda o último
`systemPrompt`/`userContent` que de fato saiu para um provedor, por turno.

## Emoção e expressão

**MUST**: a emoção sai do texto gerado (marcador do LLM ou análise) e move avatar e prosódia.

**MUST**: o marcador é removido do texto antes de virar voz ou aparecer no chat. Portado e testado
— `strip_emotion_tags`
(`apps/desktop/src-tauri/src/modules/voice/auto_speak/speech_text.rs`) roda no chamador antes de
qualquer frase ir para TTS.

**MUST NOT** escolher expressão por palavra-chave em lista fixa. É o mesmo erro do regex de
capacidade: chuta com confiança. A expressão do avatar é acionada por uma tool que o LLM chama
explicitamente (`run_expression`,
`apps/desktop/src-tauri/src/modules/features/character/tool.rs`), não por regex sobre o texto de
saída.

## Consistência de persona (evals)

**MUST (meta)**: existe um conjunto fixo de prompts, rodado contra cada perfil de personalidade e
contra cada provedor configurável ([02](02-providers.md)), cujas respostas são julgadas (por
LLM-juiz ou por heurística de similaridade de embeddings) quanto à aderência ao perfil escolhido.
Roda sob demanda — o mesmo padrão do script `smoke` do `package.json` — nunca dentro de
`pnpm gates`, porque depende de chamar um provedor de verdade.

**Estado em 2026-09-26**: não existe. Nenhuma spec ou gate mede deriva de persona ao longo de uma
sessão longa nem entre trocas de provedor/modelo.

> **Por quê:** a seção "A regra inegociável" já garante QUE toda fala é gerada por LLM, e "Perfis de
> personalidade" já garante que o perfil muda COMO ela fala, nunca O QUE ela sabe — mas nenhuma das
> duas garante que essa aderência **se mantém**. Google, Ollama e OpenRouter podem responder com
> tons bem diferentes ao mesmo `system_prompt` ([02](02-providers.md)), e uma troca de provedor no
> meio do uso pode fazer "ela virou outra pessoa" sem que nenhum teste hoje perceba.

## Horário de silêncio

**MUST**: proatividade espontânea (comentário de tela, puxar assunto, backseat gaming) respeita uma
janela de horário silencioso configurável pelo usuário, com exceção só para o que o próprio sistema
classifica como prioridade máxima.

**MUST NOT** o horário silencioso bloquear uma resposta a algo que o usuário disse diretamente —
silêncio é sobre a Mika **puxar** assunto sozinha, nunca sobre deixar de responder quem já falou com
ela.

**Estado em 2026-09-26 (Portado)**: mecanismo real, com UI. `character.quietHoursStart`/
`character.quietHoursEnd` (0-23, default 23/7, "vira a virada da noite" quando `start > end`) são
lidos em `apps/desktop/src-tauri/src/modules/brain/awareness/mod.rs:109-111` a partir da hora LOCAL
de verdade (`chrono::Local`, corrigindo um corte anterior que usava UTC e deslocava a janela). A
checagem pura (`is_quiet_hours(hour, start, end)`,
`apps/desktop/src-tauri/src/modules/brain/awareness/gates.rs`) é consultada em
`brain/awareness/engine.rs` antes de qualquer candidato proativo falar, e libera só quem tem
`priority >= 9` (testado: `quiet_hours_blocks_low_priority_but_not_priority_nine`). A UI existe —
dois sliders (início/fim) em
`apps/desktop/src/features/character/renderer/behavior-tuning-cards.tsx`, lidos e passados por
`apps/desktop/src/features/character/renderer/behavior-panel.tsx`, dentro da aba de comportamento.
