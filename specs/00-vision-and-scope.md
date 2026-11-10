# 00 — Visão e escopo

## O produto

Mika é uma companion VTuber que roda **local, no PC do usuário**, e acompanha ele enquanto joga,
programa e consome cultura nerd. Ela vê a tela, fala por voz, tem avatar Live2D, lembra do que
aconteceu e de quem são as pessoas citadas, e conversa com naturalidade.

O usuário não "usa" a Mika. Ele **convive** com ela.

## As cinco capacidades que definem o produto

Se qualquer uma some, o produto deixa de ser o que é:

1. **Visão** — ela sabe o que está na sua tela agora, sem você contar.
2. **Memória** — curto prazo (a conversa) e longo prazo (fatos, episódios, pessoas).
3. **Aprendizado** — o que ela sabe sobre você cresce sozinho, sem cadastro manual.
4. **Persona configurável** — profissional, tsundere, sarcástica ou amiga, com nível de
   relacionamento que o usuário escolhe e que evolui.
5. **Escolha de provedor** — OpenRouter, Gemini, OpenAI Compatible ou Ollama, cloud ou local.

## Não-objetivos

Estes são explícitos para impedir que alguém "melhore" o produto na direção errada:

- **MUST NOT** virar assistente de produtividade genérico. Ela ajuda porque se importa, não porque
  é uma ferramenta.
- **MUST NOT** exigir servidor, conta ou nuvem própria. Um app, um processo, dados do usuário na
  máquina dele.
- **MUST NOT** ser multi-usuário ou multi-tenant. É uma relação de uma pessoa com uma companion.
- **MUST NOT** depender de um provedor de LLM específico para funcionar. Trocar de provedor é
  configuração, nunca refatoração.
- **MUST NOT** ter fala pré-escrita. Ver [07 — Persona](07-persona-and-relationship.md).

## Perfil do usuário

Uma pessoa só, no PC dela, jogando ou programando, com a Mika em overlay. Ela pode estar de mãos
ocupadas (jogo em tela cheia) — então **voz é entrada e saída de primeira classe**, não um extra.

## O que "do jeito certo" significa aqui

Este projeto já falhou de formas específicas. Estas specs codificam as lições:

| Falha real                                                                   | Lição virou invariante em                     |
| ---------------------------------------------------------------------------- | --------------------------------------------- |
| Modelo de um provedor apontado para outro, HTTP 400 em loop infinito         | [02](02-providers.md), [03](03-resilience.md) |
| Circuit breaker completo, testado, e **não usado por nenhum caminho de LLM** | [03](03-resilience.md)                        |
| Enum de 3 modelos no schema descartando em silêncio o modelo escolhido       | [02](02-providers.md)                         |
| Abas inteiras de Settings inalcançáveis por um filtro de navegação           | [09](09-settings-and-ui.md)                   |
| Loop de captura logando 9 linhas por ciclo, para sempre                      | [10](10-observability.md)                     |
| Feature ativa no roster mas dormente em runtime por semanas                  | [01](01-architecture.md)                      |

O padrão que se repete: **código completo em um caminho que ninguém percorre**. Toda spec aqui
tem que responder "quem chama isso?" antes de descrever o que a coisa faz.
