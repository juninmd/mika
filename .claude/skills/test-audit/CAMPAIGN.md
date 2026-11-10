# Campanha de poda de testes

Poda a superfície inteira de testes de um subsistema num lote coerente: um módulo Rust
(`apps/desktop/src-tauri/src/modules/<ctx>`), uma feature (`modules/features/<ctx>` + seu
`apps/desktop/src/features/<ctx>/renderer`) ou um pacote (`packages/sdk`, `packages/sdk-ui`).
Régua de valor, retenção, evidência e validação de [SKILL.md](SKILL.md) valem em toda lane. Cada
passo termina no seu critério; não comece o próximo antes.

## 1. Baseline

Registre, no SHA atual de `feat/tauri-migration`/`main`: linhas de teste e de suporte do
subsistema, e o resultado de cada arquivo de teste (Vitest por caminho, `cargo test -- <filtro>`
por módulo, gates que o citam). Anote também `thresholds` do Vitest e o `PISO` de `check:test-reach`.
Falhas da baseline vão numa lista própria — aqui, vermelho na baseline costuma ser defeito real
ou teste que documentava o defeito, não teste velho.

Pronto quando todo arquivo de teste no escopo tem resultado de baseline registrado.

## 2. Lanes e inventário

Divida em **lanes** pela fronteira do dono em produção, não por prefixo de arquivo (ex. em `voice`:
captura/VAD, STT, TTS, barge-in, bridge IPC, UI). Inclua os casos do subsistema em fronteiras
compartilhadas: contratos IPC em `packages/sdk/src/contracts`, testes Rust que leem o TS, gates de
`scripts/checks` que o varrem, specs E2E e entradas `rust:` do manifesto de homologação.

Pronto quando cada arquivo de teste, gate e spec do subsistema pertence a exatamente uma lane.

## 3. Ledger read-only por lane

Um subagente read-only por lane (brief: nenhum git que escreva/restaure, nenhum sub-subagente).
Ele lê cada teste inteiro, inclusive tabelas de `it.each`, mais donos, entry points, chamadores,
histórico e em que passo o teste roda. Cada declaração entra num **ledger** (arquivo no scratchpad
ou `TASKS.md`) com uma marca; `it.each` é uma declaração, a menos que as linhas precisem de marcas
diferentes:

- `R` reter, nomeando o contrato e o bug que pega (mover de arquivo continua `R`, com nota);
- `F` reter o contrato mas consertar a asserção (negativo vácuo, leitura de fonte que lê de menos,
  teste que afirma o defeito);
- `C` consolidar, nomeando o dono que absorve a asserção: linha de tabela irmã, suíte de fronteira
  mais forte, teste Rust ou gate;
- `D` deletar, nomeando a prova que fica ou por que não há contrato.

Julgue pela asserção, não pelo nome. Confira o que o subagente afirmou antes de aceitar — agentes
já convergiram no diagnóstico errado aqui.

Pronto quando toda declaração da lane tem marca e linha de evidência.

## 4. Plano de camada por lane

O ledger é entrada, não lista de edição. Segunda passada read-only procura a **camada**
redundante: ex. suítes Vitest que re-testam no renderer, com bridge mockada, um contrato que o
teste Rust do comando já prova. Nomeie a suíte **guardiã** de cada contrato; prefira a fronteira
real (comando Rust, rota IPC com schema Zod de verdade) a um colaborador mockado.

Pronto quando cada lane nomeia arquivos aposentados, guardiã por contrato, asserções a carregar e
seams só-de-teste destravados.

## 5. Corte

Edite lane por lane. Mudanças em suporte compartilhado (fixtures E2E, `scripts/checks/lib.mts`,
helpers de teste) passam por um único dono, em série. Remova com cada lane os seams que ela
destrava: parâmetros de injeção, getters, exports de reset, indireções. Atualize catracas só com
remedição justificada no cabeçalho. Regras duráveis de posse de teste vão para `AGENTS.md`, tiradas
de erros que a campanha de fato achou.

Pronto quando todo plano de lane foi aplicado e as guardiãs de cada lane passam.

## 6. Revisão de preservação

Antes de declarar pronto, revisores independentes (um por grupo de fronteira) comparam a cobertura
deletada com as guardiãs: contrato que perdeu a única prova, asserção nova que não pode falhar.

Para cada contrato restaurado, uma **mutação** deliberada no dono de produção deve deixar a
guardiã vermelha. Mute uma cópia salva no scratchpad (hash antes/depois), nunca por
`git checkout`/`stash`, e restaure byte a byte.

Pronto quando toda lacuna foi restaurada ou rejeitada com evidência de fonte, e todo contrato
restaurado tem mutação capturada.

## 7. Defeitos de produto

Falha de baseline que sobrevive numa guardiã é bug. Corrija no dono em commit separado e prove
pelo fluxo real com um **controle** (a guardiã vermelha sem a correção, verde com ela). E2E só se o
usuário pedir. Discrepâncias alheias viram follow-up, não entram na campanha.

Pronto quando cada defeito corrigido tem controle vermelho e candidato verde no mesmo harness.

## 8. Reconciliar e entregar

Se a branch andou durante a campanha: arquivo que ela modificou e a campanha apagou continua
apagado; porte o contrato novo para a guardiã e confira que cada regressão nova tem casa. Rode de
novo a suíte do subsistema (escopada, não a completa sem pedido) e `pnpm lint && pnpm gates &&
pnpm typecheck`.

Entregue o relatório de [SKILL.md](SKILL.md), mais:

- linhas de teste/suporte na baseline e no fim, produção contada à parte;
- lanes, camadas aposentadas e guardiãs;
- lacunas de preservação achadas e suas mutações;
- defeitos de produto com controle e candidato.
