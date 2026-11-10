# 05 — Catálogo de pessoas

## Por que existe

Uma amiga sabe quem é o Bruno. Sem isso, toda menção a uma pessoa vira ruído e a Mika responde
genérico — que é o sintoma número um de "isso é um chatbot".

O catálogo é a memória social: quem são as pessoas da vida do usuário, como se relacionam com ele
e entre si, e o que já rolou com cada uma.

## Onde mora

No porte Tauri, o catálogo é feature tardia em
`apps/desktop/src-tauri/src/modules/features/person_catalog/` (Rust) — não em `modules/memory/`,
que cobre a memória social de fatos/relações de terceiros (ver [04](04-memory.md)). A resolução de
menção, gate de extração automática, merge/lifecycle e a UI de settings ficam todos dentro desse
diretório; a UI (`apps/desktop/src/features/person-catalog/renderer/`) é o que resta em
TypeScript, falando com o backend via `window.electronAPI`/bridge, não mais IPC do Electron.

## Modelo

Uma pessoa tem:

- **identidade** — nome canônico e apelidos (o mesmo Bruno é "Bruninho" e "BR")
- **relação com o usuário** — amigo, irmão, colega de trabalho, parceiro de jogo
- **fatos** — gosta de FPS, mora em outra cidade, trabalha com design
- **menções** — quando apareceu na conversa, em que contexto
- **relações com outras pessoas** — namorada do Bruno, irmã da Ana

**MUST**: identidade separada de apelidos. Sem isso, "Bruninho" e "Bruno" viram duas pessoas.

## Grafo de relações

Duas relações distintas compõem o modelo: **usuário↔pessoa** (o vínculo de cada pessoa com o
próprio usuário) e **pessoa↔pessoa** (o Bruno é namorado da Ana, a Maria trabalha com o João). As
duas precisam existir de verdade para "como o João conhece a Maria?" ter resposta.

**MUST**: toda aresta — usuário↔pessoa ou pessoa↔pessoa — nomeia as duas partes explicitamente,
carrega proveniência (de qual turno/menção saiu) e confiança, e reforça a confiança quando a mesma
relação é reafirmada, em vez de nascer com um valor fixo e nunca mudar.

**Estado em 2026-09-26**: parcialmente cumprido, e por dois caminhos que não se falam.

- `PersonRecord.relationship_type` (`apps/desktop/src-tauri/src/modules/features/person_catalog/types.rs`)
  é uma **escada de
  afinidade automática** (`Stranger → Acquaintance → Friend → CloseFriend → BestFriend`, calculada
  do placar de afinidade por `resolve_relationship_type`, com `Vvip`/`Creator`/`PlaysWith` travados à
  parte), não um vocabulário de parentesco/papel. Dizer "meu irmão João" não move esse campo — só a
  afinidade subindo com o tempo move. Confirmado em produção: todas as 5 pessoas reais do catálogo
  ao vivo desta rodada têm `relationshipType: "stranger"`, inclusive depois de o usuário descrever
  kinship explícito ("meu irmão João", "meu chefe Pedro") na mesma conversa.
- `memory::social::relationship_graph::add_relationship`
  (`apps/desktop/src-tauri/src/modules/memory/social/relationship_graph.rs`) grava a aresta
  usuário→pessoa pedida e, quando existe inverso conhecido (`REL_INVERSE`: namorada↔namorado,
  chefe↔funcionário, irmão↔irmão…), grava TAMBÉM uma aresta rotulada `PersonToPerson` — mas essa
  aresta reaproveita o **mesmo** `person_id` do registro original: `RelationshipEdge` não tem um
  segundo campo de pessoa. Estruturalmente ela não consegue nomear uma segunda pessoa real — "o
  Bruno é namorado de quem" nunca tem uma resposta gravável, só "o Bruno tem uma aresta rotulada
  `namorado`, direção `PersonToPerson`, sem dizer com quem".
- `confidence` existe nessa aresta (default `0.85`) mas nunca é reforçada por reaparecer — ao
  contrário de `fact_store::add_fact`, que sobe a confiança de um fato quase-duplicado (similaridade
  de Jaccard). Proveniência (de qual turno saiu) não existe — só um timestamp de atualização.
- Separadamente, `PersonRecord.relationships` (`Option<Vec<PersonRelationship>>`, campo
  `{name, kind, affinity?, trust?, description?}`) é uma segunda lista, por texto livre de nome, só
  populada pela tool `upsert` — cujo schema de entrada não documenta nenhuma propriedade esperada, e
  o prompt de sistema nunca instrui o modelo a preenchê-la. As duas listas (esta e a de
  `memory::social` acima) não compartilham id nem se sincronizam.

**SHOULD**: uma correção de relação ("não, ela é minha prima", substituindo "colega") **supersede**
a relação anterior, no mesmo padrão de contradição que fatos já seguem ([04](04-memory.md)).

**Estado em 2026-09-26**: não cumprido. `RelationshipEdge` não tem `active`/`superseded_by`
(diferente de `LearnedFact`, que tem os dois). Uma correção grava uma aresta **nova**, com id
diferente (`{person_id}:{label normalizado}`), e a anterior continua ativa ao lado dela — as duas
coexistem, sem marcação de qual venceu. Não existe também nenhuma ação de tool para atualizar uma
relação já gravada (`add_relationship`/`delete_relationship` existem; `update_relationship` não).

**SHOULD**: existe uma forma de consultar o grafo por travessia limitada (1-2 saltos) a partir de
uma pessoa — "quem o João conhece" —, nunca despejando o grafo inteiro no prompt.

**Estado em 2026-09-26**: não cumprido. `SocialMemoryService::get_graph()`
(`apps/desktop/src-tauri/src/modules/memory/social/mod.rs`) devolve o grafo **inteiro**, sem filtro
e sem paginação; não existe nenhuma função de busca em largura (BFS) nem de vizinhança em
`person_graph.rs`/`relationship_graph.rs`. A única forma de "raciocinar" sobre o grafo hoje é o LLM
ver o que já entra automaticamente no prompt — limitado por design a 3 pessoas e 200 caracteres de
bio cada (ver "Uso na conversa" abaixo), o que já cumpre o MUST NOT de não despejar o dossiê, mas não
responde a uma pergunta de travessia explícita.

## Criação automática

**MUST**: pessoas entram no catálogo **automaticamente**, a partir da conversa. O usuário nunca
precisa cadastrar ninguém para a Mika lembrar.

**MUST**: a criação automática é conservadora. Na dúvida entre criar pessoa nova e vincular a uma
existente, **vincular perde** — duplicata é corrigível, fusão errada de duas pessoas não é.

**SHOULD**: pessoa criada automaticamente nasce com confiança baixa e sobe conforme reaparece.

## Resolução de menção

Ao processar um turno, o sistema identifica pessoas citadas e injeta o que sabe delas no contexto.

**MUST**: a resolução considera apelidos.

**MUST**: menção ambígua (dois "Bruno") **MUST NOT** ser resolvida por chute. Ou a Mika pergunta,
ou trata sem vincular.

O gate de cooldown/anti-spam da extração automática (não confundir com resolução de ambiguidade) é
`MentionGate` em `apps/desktop/src-tauri/src/modules/features/person_catalog/mention_gate.rs` —
intervalo mínimo entre extrações, graça pós-escrita da tool e TTL de texto repetido.

**MUST**: a resolução acontece **antes** de montar o prompt. Depois é tarde.

**Portado**: no turno de chat regular em Rust, `persona::compose_system_prompt`
(`apps/desktop/src-tauri/src/modules/brain/persona/mod.rs::build_mentioned_people_block`) injeta as
pessoas do catálogo citadas na mensagem atual — `CatalogStore::mentioned_in`
(`features/person_catalog/query.rs::people_mentioned_in`) casa `primary_name`/apelidos contra o texto
do turno por substring case-insensitive (nunca por LLM: este caminho é síncrono e MUST NOT atrasar a
resposta — a extração por LLM do `MentionGate` continua separada, pós-turno, só para CRIAR pessoas
novas). Teto de 3 pessoas e 200 caracteres de bio cada (05-person-catalog.md §"Uso na conversa": MUST
NOT despejar o dossiê). A injeção via tool `person_catalog` chamada explicitamente pelo modelo
continua existindo à parte, para busca sob demanda além do que entra automaticamente.

## Uso na conversa

A Mika **MUST** usar o catálogo para:

- reconhecer quem é a pessoa sem pedir contexto;
- puxar assunto ("faz tempo que você não fala do Bruno");
- ligar pessoas a jogos ("o Bruno não te chamou pra CS ontem?").

**MUST NOT** despejar o dossiê. Saber muito e falar tudo é comportamento de banco de dados, não de
amiga.

## Controle do usuário

**MUST**: o usuário vê a lista completa, edita, funde duplicatas e apaga.

**MUST**: apagar uma pessoa apaga as menções e relações dela. Portado e testado —
`delete_person` (`apps/desktop/src-tauri/src/modules/features/person_catalog/mutate/lifecycle.rs`)
varre o catálogo, o grafo social e as menções episódicas juntos
(`apps/desktop/src-tauri/src/modules/features/person_catalog/tool.rs`,
`delete_sweeps_catalog_social_graph_and_episodic_mentions_together`).

**MUST**: existe um jeito de marcar alguém como "não lembre desta pessoa". Portado como
bloqueio/desbloqueio (`block_person`/`unblock_person`/`list_blocked` em
`apps/desktop/src-tauri/src/modules/features/person_catalog/mutate/moderation.rs`): bloquear apaga
o registro catalogado e impede um upsert futuro com o mesmo nome/apelido.

## Privacidade

Este é o dado mais sensível do produto — informação sobre **terceiros que não consentiram**.

**MUST**: fica local.

**MUST NOT** sair da máquina fora do contexto de um turno que exige a pessoa.

**MUST NOT** ser incluído em log, telemetria, relatório de erro ou export de diagnóstico.

**SHOULD**: o catálogo é sanitizado de qualquer payload de erro antes de qualquer envio.
**[Verificado, SHOULD não violado]** — não existe hoje, em Rust, nenhum caminho de relatório de
erro/crash/export de diagnóstico que envie propriedades arbitrárias para fora do processo (busquei
`error_report`/`crash_report`/`diagnostic_export` em toda a árvore de `src-tauri/src`: zero
ocorrências fora de teste). `sanitize_properties`
(`apps/desktop/src-tauri/src/modules/telemetry.rs`) existe e está testado, mas continua
`#[allow(dead_code)]` porque não há chamador de produção — não porque um payload de erro vaze sem
passar por ele. O risco que este SHOULD mitigaria (dado do catálogo saindo num payload de erro) é
coberto por uma garantia MAIS FORTE, descrita abaixo: PII é redigida ANTES de tocar disco, então nem
chega a existir em texto cru dentro do catálogo para vazar depois.

**Feature encontrada, não documentada aqui:** todo texto (bio, nota, menção) que entra no catálogo
passa por redação de PII **antes de tocar disco** —
`apps/desktop/src-tauri/src/modules/features/person_catalog/pii.rs`, porte de
`redactPii`/`REDACTION_PATTERNS`: 5 padrões de PII social (email, CPF, CNPJ, cartão, telefone) e 4
de segredo (chave `sk-`, token do GitHub, JWT, `key=valor`). Isto é mais forte que "sanitiza antes
do envio": o dado nem chega a ser persistido.
