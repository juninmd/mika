---
name: test-audit
description: "Invoke whenever writing, changing, reviewing, or sweeping tests in Mika (Vitest, cargo test, Playwright E2E, scripts/checks gates). Authoring gate for new tests plus audit workflow for low-value, implementation-coupled, duplicative, or defect-ratifying tests and the test-only production seams they demand."
---

# Test Audit (Mika)

Adaptada de `openclaw/.agents/skills/test-audit`. Três modos, uma régua de valor:

- **Autoria** — todo teste novo ou alterado passa pelo [portão](#portão-de-autoria) antes de existir.
- **Auditoria** — varredura focada de testes que re-afirmam o fonte, duplicam prova mais forte,
  acoplam comportamento à implementação, **ratificam um defeito** ou mantêm seams de produção que só
  teste usa. Poucos candidatos de alta confiança por rodada; mede-se confiança, não deleções.
- **Campanha** — poda a superfície inteira de testes de um subsistema (um `modules/<ctx>` Rust, uma
  feature de `apps/desktop/src/features/<ctx>`, um pacote). Antes de começar, leia
  [CAMPAIGN.md](CAMPAIGN.md).

Leia `AGENTS.md` da raiz antes de tudo: ele define as camadas e o workflow de validação que esta
skill não substitui.

## Camadas e dono de cada contrato

Cada contrato tem **um** dono de teste, na fronteira mais forte que o alcança:

| Camada | Onde | Dona de |
|---|---|---|
| E2E (Playwright) | `apps/desktop/e2e/specs/` | UI e o app montando de verdade (binário release). Só roda quando o usuário pede. |
| Vitest | `**/__tests__/*.test.ts`, `scripts/**/*.test.mts` | lógica pura do renderer, bridge, `packages/sdk*` |
| cargo test | `#[cfg(test)]` em `apps/desktop/src-tauri/src/**` | backend Rust; inclui contratos cross-language (`bridge.rs`, `permissions.rs`, `shortcuts.rs` leem o TS) |
| Gates | `scripts/checks/*.mts` via `pnpm gates` | invariantes estruturais do repositório (topologia, eventos, i18n, rede, privacidade) |

Outra camada só ganha teste do mesmo contrato se tiver um risco próprio que o dono não alcança
(transporte IPC, ciclo de vida da janela, serialização Zod na fronteira).

## Portão de autoria

O teste vem **antes** do código (`AGENTS.md` §5). Feature complexa prova-se por E2E; unidade só
para lógica pura ou contrato que o E2E não alcança barato. Antes de adicionar um teste, responda
quatro perguntas; resposta faltando = não adicione ainda:

1. Que comportamento observável, invariante ou contrato independente ele protege?
2. Que regressão crível o faz falhar?
3. Por que a cobertura existente não pega essa falha? Prefira estender um `it.each`/tabela ou
   fixture compartilhado a um teste quase-duplicado.
4. Ele exige um seam de produção (export, flag, parâmetro de injeção, getter de reset) que nenhum
   chamador de produção usa? Se sim, mova o teste para a fronteira real. Seam só-de-teste em código
   de produção fere "Zero Mocks em Features" (`AGENTS.md` §3.1).

Depois confira contra os [padrões de lixo](#padrões-de-lixo). Teste que quebra num refactor que
preserva comportamento está afirmando implementação — reescreva na fronteira dona antes de entrar.

**Regressão de bug:** tem de falhar no código pré-correção pelo motivo certo e passar depois. Uma
regressão que nunca falhou prova o dublê, não a correção. Uma por bug, na fronteira dona — não
replique o cenário em cada camada que ele atravessa.

## Padrões de lixo

Checklist dos dois modos:

- teste de unidade escrito depois do código só para cobrir linhas (`AGENTS.md` §5.1);
- unidade que não pegaria defeito real que o E2E ou o dono de fronteira já não pegue;
- spec E2E sem artefato final verificável e repetível (`AGENTS.md` §5.3);
- teste sem asserção (sonda de cobertura) ou asserção que não pode falhar;
- auto-comparação, cópia identidade, valor esperado produzido pelo próprio helper sob teste;
- fixture, inventário, manifesto ou lista de exports copiados à mão;
- grep exato de fonte/import/string **fora** de gate ou contrato cross-language;
- predicado privado testado em duplicata do que a fronteira real já prova;
- dublê que implementa o comportamento afirmado, ou um mesmo dublê fazendo papel de APIs
  diferentes (ex.: LLM dublê que fala a API do Ollama mascarando o provedor real);
- fixture que fornece a ordem de callback, o default de schema ou o store que o dono deveria
  produzir (store montado à mão esconde default que não sai no primeiro boot);
- controle negativo que passa por motivo alheio (negado por outro guard, rejeição que a produção
  nunca alcança);
- teste que lê fonte por caminho fixo e, depois de um split, lê de menos — fica verde provando menos;
- nome que promete mais que a entrada exercita; julgue pela asserção, não pelo nome;
- teste que **documenta o defeito**: afirma o comportamento errado e quebra quando o bug é
  corrigido (já aconteceu três vezes aqui);
- `vi.fn()` tipado como `never` ou `as any` que só compila porque Vitest não typecheca.

## Régua de valor e de retenção

Teste se paga protegendo comportamento, regressão crível ou contrato independente. Na auditoria,
teste que precisa mudar num refactor neutro é **suspeito**, não automaticamente deletável.

Mantenha quando ele guarda sozinho: contrato IPC (`packages/sdk/src/contracts/`), schema de config,
evento do barramento, política de privacidade/ZDR, guarda de superfície, prompt-byte, contrato
cross-language TS↔Rust, default, segurança, plataforma. Mantenha também:

- ordem de chamada quando a ordem é comportamento observável (barge-in, fila de LLM, stream de frase);
- inspeção de fonte quando é o guard independente mais barato: falha quando o contrato muda (chave,
  byte, caminho) e sobrevive a renomear identificador — é o caso dos gates e dos testes Rust que leem TS;
- teste retido que falha na baseline: trate como possível bug de produto, reproduza e corrija o dono.

Estático ou lento não é motivo de deleção.

Antes de julgar um candidato leia inteiro: o teste, o dono em produção, entry point, chamadores,
chamados, irmãos, testes sobrepostos, em que passo ele roda (`test:unit`, `test:rust`, `gates`,
E2E) e `git log --follow` do arquivo. Se afirma comportamento de dependência, leia o fonte/tipos da
dependência.

## Descoberta

Somente leitura; reporte evidência antes de editar. Para escopo amplo, lanes paralelas de
subagente (brief: **nenhum git que escreva/restaure, nenhum sub-subagente**):

- `packages/sdk`, `packages/sdk-ui`;
- renderer e bridge (`apps/desktop/src/renderer`, `apps/desktop/src/features/*/renderer`);
- Rust (`apps/desktop/src-tauri/src`);
- gates (`scripts/checks`) e E2E (`apps/desktop/e2e`);
- varredura transversal por padrão de lixo (script por classe rende mais que ler arquivo).

## Evidência por candidato

Preencha tudo antes de editar; campo faltando = candidato não pronto:

- nome exato do teste e `arquivo:linha`;
- que falha ele de fato detecta;
- chamadores não-teste do seam coberto;
- prova mais forte que continua no dono, ou por que nenhuma é necessária;
- histórico e motivo de existir (cabeçalho do gate, commit, memória);
- deleção de produção/suporte que ele destrava;
- risco e o comando de validação focada.

## Forma da edição

Um lote coerente por fronteira dona. Apague exports, globais, wrappers e caminhos de produção que
só teste usa, sem deixar alias. Mova regressões retidas para o dono canônico. Prefira LOC de
produção líquido negativo. Não crie teste substituto que re-afirma a mesma implementação nem
transforme candidato incerto em limpeza para inflar contagem.

**Catracas que só sobem:** `thresholds` em `vitest.config.mts` e `PISO` em
`scripts/checks/test-reach.mts`. Deletar teste que derruba uma delas exige provar que o alcance
perdido era de fonte também removida — e remedir o piso com a justificativa no cabeçalho, como os
registros de 2026-09-22/23. Nunca abaixe catraca para ficar verde.

## Validação

Nunca edite fonte ou teste com Vitest em watch rodando. Modo quiet sempre.

1. Menor escopo dono + irmãos:
   `pnpm exec vitest run <caminho> --reporter=dot` e/ou
   `cargo test --quiet --manifest-path apps/desktop/src-tauri/Cargo.toml --lib -- <filtro>`
   (`cargo check` não compila teste: verde falso).
2. Removeu inspeção de fonte ou gate: rode o gate/script dono do contrato real (`pnpm -s check:<x>`).
3. Mutação de prova: nunca edite-e-reverta arquivo rastreado à mão. Copie o original para o
   scratchpad, registre o hash, mute, rode, copie de volta e confira o hash (a árvore tem centenas de
   arquivos não commitados; `git checkout --`/`stash`/`restore` estão proibidos).
4. `pnpm lint`, `pnpm gates`, `pnpm typecheck` (checar exit code, não grep), `git diff --check`.
5. `git diff --numstat`: reporte produção/tooling separado de teste e suporte de teste.
6. E2E só se o usuário pedir, e aí só os specs afetados (após `tauri build --no-bundle`).
7. Diff multi-arquivo: revisão independente (subagente novo) antes de declarar pronto.

## Commit e continuação

Commit só dos arquivos desta rodada (`git diff --cached --name-only` antes), um lote por commit,
prefixo `test:`/`refactor:` com o porquê no corpo. Sem push. Depois de cada lote, refaça a
descoberta read-only para o próximo.

## Entrega

Use o relatório padrão (veredito + tabela) e inclua nele:

- causa raiz e categorias de baixo valor removidas;
- simplificações no dono de produção;
- falsos positivos retidos e por que valem;
- prova focada e completa efetivamente rodada;
- LOC de produção vs teste; catracas remedidas;
- bugs de produto achados (teste que ratificava defeito) e follow-ups nomeados.
