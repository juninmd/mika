# Specs — Mika

Especificação de referência para construir a Mika **do jeito certo**: uma VTuber AI companion de
gaming que enxerga a tela, lembra de você e das pessoas da sua vida, aprende com o tempo, e
conversa como uma amiga — não como um assistente.

## Como ler

Estes documentos definem **o que o sistema deve ser**, com invariantes verificáveis. Não são
tutorial nem histórico. Quando uma spec conflita com o código, a spec ganha — ou a spec está
errada e deve ser corrigida no mesmo commit.

| #   | Documento                                                  | Responde                                                                     |
| --- | ---------------------------------------------------------- | ---------------------------------------------------------------------------- |
| 00  | [Visão e escopo](00-vision-and-scope.md)                   | O que a Mika é, o que ela nunca será                                         |
| 01  | [Arquitetura](01-architecture.md)                          | Onde cada capacidade mora e por quê                                          |
| 02  | [Provedores](02-providers.md)                              | Os 4 provedores, credenciais, catálogo de modelos                            |
| 03  | [Resiliência](03-resilience.md)                            | Circuit breaker, falha fatal vs transiente, saúde na UI                      |
| 04  | [Memória](04-memory.md)                                    | Curto prazo, longo prazo, embeddings, aprendizado                            |
| 05  | [Catálogo de pessoas](05-person-catalog.md)                | Quem é quem na sua vida                                                      |
| 06  | [Visão](06-vision.md)                                      | Screen watcher, custo, o que ela comenta                                     |
| 07  | [Persona e relacionamento](07-persona-and-relationship.md) | Perfis, níveis de intimidade, fala                                           |
| 08  | [Gamer companion](08-gamer-companion.md)                   | O que ela faz enquanto você joga                                             |
| 09  | [Configuração e UI](09-settings-and-ui.md)                 | Como o usuário configura sem se machucar                                     |
| 10  | [Observabilidade](10-observability.md)                     | Logs que servem para diagnosticar                                            |
| 11  | [Qualidade](11-quality-gates.md)                           | O que precisa passar antes de dizer "pronto"                                 |
| 12  | [Avatar: Live2D e VRM](12-avatar-render.md)                | Como ela se move: ações geradas, máscara, captura                            |
| 13  | [Voz e turno de fala](13-voice-and-turn-taking.md)         | Ouvir e falar como um só laço: STT, barge-in, o que nunca é falado           |
| 14  | [Harness de agente](14-agent-harness.md)                   | Tool loop, aprovação, o que ela nunca pode reportar como feito sem ter feito |

## Convenções

- **MUST / MUST NOT** — invariante. Violação é bug P0, quebra gate de CI ou deve quebrar.
- **SHOULD** — forte recomendação; desvio precisa de justificativa escrita no código.
- **MAY** — escolha livre.

Cada invariante que já foi violado na prática carrega uma nota `Por quê:` com o incidente real.
Essas notas existem porque um invariante sem história é o primeiro a ser removido por alguém que
acha que ele não faz nada.

## Princípio que organiza todo o resto

> A Mika é uma **presença**, não uma interface. Tudo que ela diz é gerado por LLM, com contexto de
> quem você é, do que está na tela, e do que ela lembra. No momento em que ela responde com uma
> string constante, ela virou um chatbot de FAQ e o produto morreu.

Todo o resto destas specs — memória, visão, persona, provedores, resiliência — existe para
sustentar essa frase.
