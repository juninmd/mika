# Migração Electron → Tauri **sem sidecar Node**

> **REGISTRO HISTÓRICO.** Diário da migração Electron → Tauri/Rust (Stage C concluído em 2026-09-22/23: sidecar Node, `packages/llm`, `packages/mood` e `packages/core-native` apagados). As tabelas de equivalência de crates seguem úteis; o "Estado: em execução" abaixo, os caminhos `src/main/**` e as listas de pendências NÃO refletem o repositório atual. A arquitetura vigente está em `AGENTS.md` §2.

Estado: em execução, branch `feat/tauri-migration`.

O objetivo mudou em 2026-09-21: não basta o shell ser Tauri. O backend inteiro, hoje ~52k LOC de
TypeScript rodando num sidecar Node, vira Rust. O sidecar deixa de existir.

Este documento existe porque a pesquisa de equivalências já foi feita uma vez e **se perdeu por só
ter existido no contexto de uma conversa**. O que está aqui foi verificado contra o registro de
crates, não contra memória.

---

## 1. Equivalências verificadas

Versões conferidas em `cargo search` em 2026-09-21. O que não está aqui ainda não foi verificado e
não deve ser assumido.

- **Node (hoje):** `ai`, `@ai-sdk/openai`, `@ai-sdk/google`, `@openrouter/ai-sdk-provider`, `ai-sdk-ollama`
  - **Rust:** `rig-core`
  - **Versão:** 0.42.0
  - **Observação:** Tem streaming, `ToolCallDelta` e laço multi-turn (`MultiTurnStreamItem`), que é o que `brain` usa
- **Node (hoje):** `@ai-sdk/mcp`
  - **Rust:** `rmcp`
  - **Versão:** 3.4.0
  - **Observação:** SDK oficial de MCP em Rust
- **Node (hoje):** `onnxruntime-node`
  - **Rust:** `ort`
  - **Versão:** 2.0.0-rc.13
  - **Observação:** Já decidido: `tract-onnx` foi REPROVADO, não roda nem o Silero VAD
- **Node (hoje):** `@huggingface/transformers`
  - **Rust:** `ort` + `tokenizers`
  - **Versão:** — / 1.0.0-rc.2
  - **Observação:** transformers.js é runtime ONNX + tokenizer; os dois lados existem
- **Node (hoje):** `playwright` (serviço `browser`)
  - **Rust:** `reqwest` + `scraper`
  - **Versão:** 0.12 / 0.27.0
  - **Observação:** DECIDIDO em 2026-09-22 (§5): sem motor de browser — ver a decisão abaixo. `chromiumoxide` foi removido do `Cargo.toml`, nunca teve chamador
- **Node (hoje):** `@libsql/client`
  - **Rust:** `libsql` **ou** `rusqlite`
  - **Versão:** 0.10.0-pre.4 / 0.40.2
  - **Observação:** DECISÃO ABERTA — `libsql` está em pré-lançamento; se o uso for SQLite local puro, `rusqlite` estável ganha
- **Node (hoje):** captura de tela (`vision`)
  - **Rust:** `xcap`
  - **Versão:** 0.9.8
  - **Observação:** X11, Wayland, macOS e Windows
- **Node (hoje):** processo em primeiro plano (`gamer-companion`)
  - **Rust:** `sysinfo` + `windows`
  - **Versão:** 0.39.6 / 0.62.2
  - **Observação:** a parte de janela em foco é por SO
- **Node (hoje):** `espeak-ng`
  - **Rust:** `espeakng`
  - **Versão:** 0.2.0
  - **Observação:** wrapper sobre espeakNG-sys
- **Node (hoje):** áudio (captura, resample, wav, playback)
  - **Rust:** `cpal`, `rubato`, `hound`, `rodio`
  - **Versão:** 0.18.2 / 5.0.0 / 3.5.1 / 0.22.2
- **Node (hoje):** `electron-updater`
  - **Rust:** `tauri-plugin-updater`
  - **Versão:** —
  - **Observação:** plugin oficial
- **Node (hoje):** `adm-zip`
  - **Rust:** `zip`
  - **Versão:** 9.0.0-pre3
- **Node (hoje):** `p-map`, `p-retry`, `opossum`
  - **Rust:** `tokio`, `backoff`, `failsafe`
  - **Versão:** —
  - **Observação:** concorrência, retry e circuit breaker
- **Node (hoje):** segredos
  - **Rust:** `keyring`
  - **Versão:** 4.2.0

**Fica em JavaScript:** todo o `renderer` (~31k LOC). React, Pixi, three-vrm, Live2D e mermaid são o
frontend web que o Tauri serve — migrar isso não estava no pedido e não traz nada.

---

## 2. Estratégia: o sidecar morre por atrito, não por substituição de uma vez

`bridge.rs` já decide, por canal, entre resolver no Rust e encaminhar ao sidecar — é assim que os 6
canais de shell já funcionam hoje. A migração generaliza essa bifurcação:

```
renderer → mika_invoke(canal, payload)
             ├── canal registrado nativo?  → módulo Rust
             └── senão                     → sidecar Node   ← esta seta encolhe até sumir
```

Consequências, e é por isso que este é o caminho:

- **O app funciona em todo commit.** Nunca existe um estado "metade portada, nada roda".
- **A unidade de trabalho é o módulo**, e cada um é verificável sozinho.
- **O progresso é contável**, não uma sensação: o número de canais ainda encaminhados só pode cair.
  Isso vira gate (`scripts/checks/sidecar-attrition.mts`), com catraca — igual à de cobertura.
- O contrato IPC (`packages/sdk/src/contracts/`) **não muda**. O renderer não sabe que o outro lado
  trocou de linguagem, então nenhuma feature se perde por acidente de transporte.

## 3. Ordem de migração

Por dependência e por risco, não por tamanho. Cada fase termina verde nos gates do AGENTS.md.

**A ordem abaixo foi corrigida em 2026-09-21.** A primeira versão começava pelos módulos menores
(`telemetry`, `update`, `desktop`), por serem baratos. Ler `telemetry` derrubou isso: ela depende de
`IConfigStore` **e** do banco. Módulo pequeno não é módulo independente, e 62 arquivos do backend
leem configuração — enquanto ela viver no sidecar, qualquer outro módulo "migrado" ou leria config
pelo IPC de volta ao Node (dependência ao contrário) ou teria duas fontes da verdade.

- **#:** 0
  - **Fase:** Registro nativo + catraca
  - **LOC TS:** —
  - **Por que aqui:** sem ela não há como provar progresso nem impedir regressão
  - **Estado:** ✅
- **#:** 1
  - **Fase:** **config store** (schema, validação, persistência)
  - **LOC TS:** ~1200
  - **Por que aqui:** alicerce de 62 arquivos; nada real migra antes dele
  - **Estado:** ✅ 15 namespaces, 233 campos, paridade conferida campo a campo
- **#:** 1b
  - **Fase:** cripto do config (AES-256-GCM + scrypt)
  - **LOC TS:** —
  - **Por que aqui:** o store RECUSA caminho sensível até existir; precisa ler o formato atual ou o usuário perde as chaves
  - **Estado:** ✅
- **#:** 2
  - **Fase:** `telemetry`, `update`, `desktop` + `system`, `health`, `settings`, `llm`
  - **LOC TS:** 3122
  - **Por que aqui:** agora sim: já têm onde ler config
  - **Estado:** ✅ 26 canais, catraca em 17. `privacy` (chokepoint) continua aberto
- **#:** 3
  - **Fase:** `memory` (+ decisão libsql/rusqlite)
  - **LOC TS:** 2971
  - **Por que aqui:** banco; decide o armazenamento antes de quem escreve nele
  - **Estado:** ✅ **rusqlite** — store KV portado; episódico/social esperam llm
- **#:** 4
  - **Fase:** `llm` + `chat`
  - **LOC TS:** 3705
  - **Por que aqui:** `rig-core` entra aqui, isolado, antes do `brain` depender dele
  - **Estado:** ✅ `llm` pronto; `chat` — os 4 canais `mika:chat-*` reivindicados (`send`/`history`/`clear`/`cancel`), histórico persistido em rusqlite no mesmo `chat.db` do TS (adota a sessão existente no 1º boot) e registro de turno por janela
- **#:** 5
  - **Fase:** `inference` + `voice`
  - **LOC TS:** 9637
  - **Por que aqui:** `ort`, `tokenizers`, `cpal`; o VAD já tem precedente em Rust
  - **Estado:** 🟡 `mic-state` e `mika:voice-transcribe` portados (Whisper por `ort`); barge-in, detector de interrupção e pipeline de VAD portados. **TTS é o que resta**, e é ele que destrava `voice:vad-audio` — ver abaixo
- **#:** 6
  - **Fase:** `vision`
  - **LOC TS:** 4823
  - **Por que aqui:** `xcap`
  - **Estado:** 🟡 `vision_capture(frame)` ponta a ponta, com chokepoint de privacidade no adaptador e blocklist de janelas. Das 6 actions, 5 dependem de LLM multimodal: degradam com `errorCode` nomeado
- **#:** 7
  - **Fase:** `brain`
  - **LOC TS:** 11685
  - **Por que aqui:** o maior, e só faz sentido depois de llm/memory/voice existirem
  - **Estado:** ✅ laço de turno em Rust, com `TurnAbortHandle` provado em teste
- **#:** 8
  - **Fase:** features restantes (`gamer-companion`, `person-catalog`, `character`, `mood`, `scheduler`, `mcp`)
  - **LOC TS:** 14700
  - **Estado:** 🟡 `mood`, `desktop` (tool) e `gamer-companion` (Steam pelo disco, sem API key) portados; `mcp` sobre `rmcp`, com toda tool negando execução até o gate de aprovação existir. `person-catalog`, `character`, `scheduler`, `user` abertos
- **#:** 9
  - **Fase:** `browser`
  - **LOC TS:** 478
  - **Por que aqui:** `reqwest` + `scraper`; isolado, pode andar em paralelo
  - **Estado:** ✅ `search`/`fetch_page` reais (decisão do §5 tomada), com o mesmo chamador que reivindicou a tool (`brain/tools/web.rs`)
- **#:** 10
  - **Fase:** Remoção do sidecar
  - **LOC TS:** —
  - **Por que aqui:** apaga `src/sidecar`, `build-sidecar.mts`, `externalBin`, e o E2E do boot do sidecar

### 3.1. Os canais que sobraram, e o que cada um espera

Medido em 2026-09-21. A catraca saiu de 43 e **parou de propósito** nos que dependem de registros
que só populam quando os módulos EXECUTAM — um registro parcial é pior que um vazio, porque apaga
tool e aba de settings da UI sem erro nenhum. `pnpm -s check:sidecar-attrition` lista os nomes e o
placar corrente.

- **Canal:** ~~`mika:tools-list`, `mika:tools-list-by-plugin`~~
  - **Espera:** **Reivindicados em 2026-09-22** (`modules/tools/native.rs`). Ver §3.3 para por que a espera terminou sem a tabela de degradação ter esvaziado
- **Canal:** ~~`mika:tool-execute`, `tool-approval-response`~~
  - **Espera:** **Reivindicados em 2026-09-22**, juntos — `bridge.rs` roteia por canal INTEIRO, e separá-los penduraria toda aprovação até o backstop. `native.rs::execute` cria a pendência e `native.rs::resolve_approval` a resolve, pelo `OnceLock` de `window_approval.rs`; `access.rs` porta a política de superfície e o risco por AÇÃO (`desktop:launch` ≠ `desktop:window`)
- **Canal:** ~~`mika:plugin-rpc`~~
  - **Espera:** **Reivindicado em 2026-09-22** (`modules/plugins/native.rs`). 24 dos 25 métodos despacham de verdade — `person_catalog_image` fica sem handler de propósito. O bloqueio nunca foi "é código, não dado para portar": era o DENOMINADOR, e ele encheu quando os quatro donos foram portados
- **Canal:** `mika:plugin-test`
  - **Espera:** **Em porte, 2026-09-22.** Os 3 embutidos (voice/llm/embeddings) prontos em `settings/plugin_test.rs`; faltam as 8 implementações genéricas de `testConnection`, uma por feature (`desktop` não tem nenhuma)

`mika:settings-declarations` saiu desta tabela em 2026-09-21: foi reivindicado quando o catálogo
fechou em 13 de 13 abas (commit `d7eca8d2`). A linha que ele ocupava dizia "deliberadamente não
ligado enquanto faltar aba", e continuou dizendo isso por dois dias depois de a aba existir — o
mesmo envelhecimento de cabeçalho que já refutou seis premissas nesta migração.

### 3.2. Por que os canais de tool não caem só com o catálogo cheio

O placar de canais mede **quem serve a rota**, não **quão bem**. Em 2026-09-21 as 24 tools do TS
passaram a ter as 24 contrapartes registradas em Rust, e isso NÃO destravou `mika:tools-list`: uma
tool registrada pode ter metade das ações respondendo `errorCode="unavailable"`. Para o modelo, a
diferença entre "a ferramenta não existe" e "a ferramenta existe e recusa" é grande e ruim — no
primeiro caso ele escolhe outro caminho, no segundo ele gasta o turno e a Mika anuncia que não
consegue.

A medição que vale é, por tool, **quantas AÇÕES despacham de verdade contra quantas o TS declara** —
diferença dos conjuntos nos dois sentidos, nunca o total. Duas dessas lacunas fecharam em
2026-09-22 (`avatar`: `expression` e `inspect_model`; `voice`: `listen_start`/`listen_stop`), as
duas por medir a premissa em vez de aceitá-la: nenhuma precisava do que o cabeçalho dizia que
precisava.

### 3.3. E por que eles caíram assim mesmo — 2026-09-22

O §3.2 acima estava certo sobre o CUSTO e errado sobre a DECISÃO, e a diferença é o objetivo.

O argumento era: tool que existe e degrada é pior que tool que não existe, logo não reivindicar.
Ele trata o sidecar como uma alternativa disponível. Mas o objetivo é **apagar o sidecar**, e
enquanto qualquer um destes 4 canais descesse para o Node o processo inteiro tinha de continuar
vivo — com tudo que ele serve junto. A escolha real nunca foi "Rust incompleto × Electron
completo": era "Rust incompleto × manter o processo Node inteiro por causa de um punhado de ações".

O que se fez com o punhado: atacá-lo, em frentes paralelas, uma por módulo, em vez de usá-lo como
motivo para adiar. O que sobrar delas continua degradando com `errorCode` nomeado — que é o que o
próprio TypeScript faz quando a peça de baixo não responde.

**Reivindicados nesta rodada**, com o que destravou cada um:

- **Canal:** `voice:vad-audio`
  - **O que era o bloqueio, e por que caiu:** Esperava o TTS: `SPEECH_STARTED`/`SPEECH_FINISHED` só saíam do `tts-speaker.ts`, e sem eles o `InterruptionDetector` nunca saía de "não falando". O TTS foi portado (`voice/tts/`), `chat::turns::abort_turn` deu o hook público de cancelamento, e o barge-in agora publica `ForceStopTts` **e** aborta o turno de LLM
- **Canal:** `mika:plugin-ui-catalog`
  - **O que era o bloqueio, e por que caiu:** Estava listado como "mesmo bloqueio de `settings-declarations`" por virem do mesmo `FeatureRegistryService`. Errado: `getUiCatalog` lê `views`/`settingsPanels`, nunca `settings`. Só `gamer-companion` e `person-catalog` declaram views em todo o TS, e os dois estão portados. **Registro compartilhado não é bloqueio compartilhado — a pergunta é qual CAMPO o canal lê**

### O catálogo de abas, medido em 2026-09-21 — e por que "quantas faltam" não é a pergunta

Três números diferentes circularam para o mesmo catálogo: 14, 15 e 13. Nenhum veio de `grep`
confiável, porque `character`/`user`/`mood` registram a aba DINAMICAMENTE (via
`features/settings_declaration.rs`) e não têm literal `tabId` no fonte para achar. Medido ao vivo
(probe descartável chamando as funções de registro sobre um `ConfigStore` de verdade, depois
removido):

- **TS declara 13 abas** — uma por `defineFeatureSchema({ … id: '…' })`, e `voice.schema.ts` declara
  DUAS (`voice` e `stt`): `brain`, `browser`, `character`, `embeddings`, `gamer-companion`, `llm`,
  `mcp`, `person-catalog`, `privacy`, `stt`, `user`, `vision`, `voice`.
- **Rust publicava 12** depois de `voice::feature::register_feature()` entrar: `character`,
  `gamer-companion`, `llm`, `mcp`, `mood`, `person-catalog`, `privacy`, `scheduler`, `stt`, `user`,
  `vision`, `voice`.

**12 contra 13 esconde o defeito real: os conjuntos não se sobrepõem.** Faltavam `brain`, `browser`
e `embeddings` (3), e sobravam `mood` e `scheduler` (2) — dois namespaces que só o Rust declara.
Contar só o total dava "falta 1" e teria reivindicado o canal apagando TRÊS telas do usuário. A
pergunta certa nunca é quantas faltam, é **quais** — a diferença dos dois conjuntos, nos dois
sentidos.

`embeddings` era o mais caro dos três: `brain` e `browser` já tinham namespace no
`config/schemas/`, `embeddings` não tinha nenhum.

Dois pontos de registro de tool, e escolher o errado não dá erro — dá catálogo vazio com binário
verde: `modules::all()` roda dentro de `native::registry()`, um `OnceLock` construído ANTES de o
Tauri montar o app (sem `AppHandle`); tool que lê config vai em
`modules::register_config_bound_tools(app)`, chamada de `lib.rs::setup`. Alargar `ToolHandler::call`
para receber `AppHandle` seria a saída errada: acoplaria todo handler testável a um app Tauri
inteiro. Toda função de registro precisa de teste-âncora provando que o boot a CHAMA — chamador
ausente é só `warning: never used`.

## 4. Regras que valem para toda fase

- **Nenhuma feature se perde.** O critério não é "compila", é: a rota responde o mesmo schema Zod
  que o contrato declara, e a UI que a consome continua viva.
- **Paridade provada por teste, não por leitura.** Cada módulo portado leva teste Rust; onde o
  comportamento era sutil, o teste TS que já existia vira o roteiro.
- **`ort` é a única opção de ONNX.** `tract-onnx` está reprovado e documentado — não reabrir.
- **Multiplataforma continua obrigatório.** Nada de `#[cfg(windows)]` sem o braço dos outros dois,
  mesmo que esta máquina só compile um.
- **Um `ConfigStore` por processo.** Peça `crate::state::config_store(app)` — nunca
  `ConfigStore::open` num `OnceLock` do módulo. O store guarda o JSON inteiro em cache e salva o
  documento completo a cada `set`: duas instâncias são dois caches, e a segunda a salvar apaga a
  escrita da primeira, sem erro e sem log. Três frentes caíram nisso ao mesmo tempo, cada uma
  correta sozinha e verde nos próprios testes. Quem impede a volta é
  `state::tests::single_config_store_per_process`, que lê o fonte dos módulos.
- **`configPath` manda, não o grupo onde o campo é declarado.** O Zod declara em um namespace e
  aponta o armazenamento para outro (`voice.elevenLabsApiKey` → `apiKeys.elevenLabs`). O port tem
  de reproduzir o caminho de ESCRITA; copiar a estrutura de declaração grava a credencial onde
  leitor nenhum consulta, e some com a chave que o usuário já tem.
- **`STATUS_ENTRYPOINT_NOT_FOUND` no `cargo test` não é o seu diff, e não é disputa de build-dir.**
  A árvore importa `TaskDialogIndirect`, que só existe no comctl32 v6 e só carrega por manifesto; o
  `build.rs` declara a dependência para o harness de teste. Se voltar, o caminho que resolve é ler
  a tabela de importação do PE e resolver símbolo a símbolo — não retry, não `cargo clean`. Sete
  frentes gastaram tentativas culpando o cache compartilhado, e uma delas apagou 3,2 GiB dele.

## 5. Dívida herdada, a resolver dentro do caminho

- **O placar de canais esconde a dívida que o usuário sente: os PROVEDORES.** Um canal reivindicado
  conta 1 no placar tendo portado um provedor ou sete. O canal de voz é nativo desde 2026-09-21, e
  com ele o sidecar deixou de servir voz — mas `voice/tts/dispatch.rs` corta SETE provedores
  (`elevenlabs`, `openai-tts`, `azure-tts`, `google-tts`, `piper`, `kokoro`, `system`) e o STT em
  nuvem (`groq-whisper`, `openai-whisper`) não tem porte. Quem usava qualquer um deles perdeu a
  feature no dia da reivindicação, não no dia em que o sidecar for apagado.
  **Consequência para a ordem das fases:** apagar o sidecar NÃO piora isso — `bridge.rs` roteia por
  canal inteiro, então manter o processo Node vivo já não devolve nenhum desses provedores. A dívida
  é independente da catraca e se paga em Rust de qualquer jeito. Em porte desde 2026-09-22: os 4
  remotos de TTS e o STT em nuvem, todos passando por `privacy::assert_network_allowed` antes do
  request. `piper`/`kokoro` (ONNX local) e `system` (TTS do SO) continuam nomeados.
- **`person_catalog_image` fica sem handler de propósito.** Buscar foto de pessoa por URL pede um
  subsistema de fetch HTTPS guardado contra SSRF a cada hop (o que `guarded_fetch/` já faz para o
  `browser`) mais cache em disco com teto de MIME e de tamanho. Sem handler, `rpc::dispatch` responde
  `handler-missing` e `use-person-image.ts` já trata como "sem foto"; uma versão fraca do fetch seria
  pior que a lacuna nomeada.
- **`mini-game:result` calcula e não publica.** Falta a variante `GameMoment` em
  `crate::events::EventKey` (literal TS `'steam:game-moment'`). A regra da casa é consumidor real
  antes da chave, e os 4 consumidores do TS (`game-moment-commentary.ts`, `slang-context-provider.ts`,
  ~~`apps/desktop/src/main/services/memory/experience.ts`~~,
  ~~`apps/desktop/src/main/services/vision/processing/observation-publisher.ts`~~) não estão portados. A chave entra junto
  com o primeiro deles, não antes.

- **DECIDIDO em 2026-09-22 — o `browser` não tem motor de navegador.** Esta seção listava três
  saídas (empacotar um Chromium, exigir Chrome do sistema, manter o Playwright via sidecar só para
  este módulo). O usuário escolheu uma QUARTA: nenhuma delas. `search`/`fetch_page` são
  `reqwest` + `scraper` (HTML parsing por seletor CSS sobre `html5ever`) — sem motor JS.
  Motivo: tamanho do instalador inalterado, zero dependência de ambiente (não pede Chrome/Edge
  instalado, não baixa ~150MB no primeiro boot), e sem sidecar Node — que é exatamente o que esta
  migração inteira existe pra eliminar. `chromiumoxide` (que chegou a ser declarado no `Cargo.toml`
  numa rodada anterior, sem chamador) SAIU: ele não empacota Chromium sozinho, só dirige por CDP um
  binário já instalado — teria mudado o requisito de ambiente do app inteiro, e a decisão foi não
  pagar esse preço.
  **Custo aceito e nomeado:** sem motor JS, uma página que só renderiza conteúdo via JavaScript
  rende MENOS texto do que o Playwright conseguia (ele executava o JS antes de ler `innerText`).
  Busca cobre os dois motores configuráveis via os endpoints sem-JS confirmados nesta rodada
  (`html.duckduckgo.com/html/`; `search.yahoo.com/search`, atrás de um bounce de verificação
  same-origin que o cookie jar do cliente HTTP atravessa). O núcleo de segurança de
  `navigation-guard.ts` (revalidação de SSRF/modo-offline a cada hop de redirect) foi portado em
  `modules/browser/guarded_fetch/` — ver o cabeçalho de `modules/browser/mod.rs` pro detalhe
  completo da decisão e do que cada arquivo cobre.
- **Os canais que sobram não são os mais fáceis adiados.** Todos dependem de estado vivo do
  processo Node: registro de tools populado por código de plugin no boot, modelos de voz que
  ninguém carregou em Rust. (O grupo `mika:chat-*` já era o exemplo clássico disso — histórico
  como objeto em memória do processo Node — até ganhar sua própria sessão em memória do lado Rust,
  ver fase 4.) Cada recusa está documentada no cabeçalho do módulo correspondente, com o defeito
  que a alternativa produziria. Baixar a catraca agora exige portar fase, não canal.
- **[Corrigido 2026-09-25] Guarda de SSRF dos endpoints digitados.** `privacy::assert_user_endpoint_allowed`
  porta o `validateUrl` do TS (loopback, ou http(s) com IP público e DNS resolvendo só para público)
  e sempre chama `privacy::assert_network_allowed`
  no caminho — são perguntas diferentes, e o teste `every_external_url_goes_through_the_offline_gate`
  cobra a segunda.

- **`inspect_model` acha os modelos em dev e não num build empacotado — e a decisão é de
  empacotamento, não de código.** O Rust já procura pelo caminho certo
  (`app.path().resolve("renderer", BaseDirectory::Resource)`, o primeiro candidato de
  `character/inspect_model.rs::candidate_base_dirs`); o que falta é `tauri.conf.json` declarar os
  assets em `bundle.resources`, que hoje só tem o sidecar. Eles chegam ao app pelo `frontendDist`,
  que o Tauri embute no binário — o webview desenha o avatar normalmente, mas o backend não tem
  arquivo em disco para ler, então a tool degrada só no instalador, nunca em `pnpm dev` nem em
  `cargo test`. **O preço é 40 MB** (7,7 MB de `live2d/` + 32 MB de `vrm/`) duplicados no
  instalador, já que continuariam também no `frontendDist`. As saídas são declarar e pagar, ou tirar
  os modelos do `frontendDist` e servi-los por `asset:` a partir de `resources` — a segunda não
  duplica, mas mexe em como o renderer carrega o avatar. Decisão do usuário.
- **`vrm_inventory` lê os 32 MB do `.vrm` inteiro para olhar o cabeçalho.** Precisa de 12 bytes de
  header + 8 de chunk + o chunk JSON; `read_model_file` faz `fs::read` do arquivo todo porque o TS
  faz o mesmo. É paridade fiel e não é caminho quente (a tool roda quando a Mika se descreve), mas
  um `Seek` de duas passadas trocaria dezenas de MB por alguns KB por chamada.
- `boot.spec.ts` (E2E do Tauri) estoura `script timeout` na chamada que atravessa o IPC. A
  investigação está em `[[project_tauri_invoke_antes_do_ready]]`. **A fase 10 apaga a causa raiz
  candidata** (o boot paralelo shell↔sidecar), então este item se resolve ou se reclassifica lá.
- A fila pré-boot em ~~`apps/desktop/src/sidecar/transport.ts`~~ — **morreu junto com o sidecar em
  2026-09-22.** Não existe mais corrida shell↔sidecar para enfileirar: `mika_invoke` chama o módulo
  nativo no mesmo processo, e canal sem dono vira erro nomeado em vez de esperar um processo subir.
- **O poller de energia ficou sem consumidor** (o porquê está no cabeçalho de
  `src-tauri/src/power.rs`). `power-suspend`/`power-resume`/`power-idle-tick`/`power-on-battery` só
  tinham consumidores no lado Node, e nenhum deles está portado. `snapshot` continua real (tem
  chamador em `modules/system`); o que sumiu é o empurrão periódico, e com ele a detecção de
  suspend/resume. Revive junto com o primeiro consumidor — a proatividade por ocioso é o candidato
  óbvio — e as chaves entram em `events/keys.rs` com ele, não antes.

### Capacidades que perderam o chamador quando o sidecar caiu — 2026-09-22

`shell_actions.rs` era a superfície que o backend Node pedia ao shell: 26 braços, dos quais 23
existiam só para o processo que foi apagado. Foram removidos em vez de ficarem dormentes — braço
sem chamador é capacidade fingida, que é a leitura que os gates deste repo existem para impedir.
O que era duplicata de caminho nativo não perdeu nada (`capture::get_sources`, `images::*`,
`updater::*`, `power::*`, `tray::*` e `shortcuts::register_app_shortcuts` continuam com chamador
próprio). O que perdeu consumidor de verdade está listado aqui, e cada item só volta com um
chamador junto:

- **Diálogos nativos** (`pickFile`, `pickDirectory`, `saveFile`, `showMessageBox`). Reinvestigado em
  2026-09-22 (rodada "capacidades sem chamador"): `IDialogAdapter`/`createDialogAdapter` em
  ~~`packages/sdk/src/host/adapters.ts`~~ + ~~`apps/desktop/src/main/infrastructure/adapters/system-adapters.ts`~~
  são registrados em `adapter-registry.ts` mas **nunca invocados fora de teste** — `grep` de
  `IDialogAdapter\|createDialogAdapter\|\.dialog\.` no repo inteiro só bate em declaração, registro
  e `__tests__`. A única UI que abre arquivo hoje usa `<input type="file">` (`BackupPanel.tsx`). Já
  era código morto do lado Node antes do sidecar cair. **Não portado** — reviver = canal no
  contrato de IPC + chamador de verdade, nessa ordem, e só quando existir.
- **Notificação do SO** (`showNotification`). Reinvestigado em 2026-09-22: `INotificationAdapter`/
  `createNotificationAdapter` (mesmos dois arquivos acima) também não têm chamador de produção —
  `new Notification(` só aparece em `system-adapters.ts:10`, e `adapters.notification.show(` só em
  ~~`packages/sdk/src/host/__tests__/headless-adapters.test.ts`~~. O scheduler (~~`packages/scheduler/src/services/scheduler.service.ts`~~)
  e a proatividade (`main/services/brain/awareness/*`) publicam `Events.NOTIFICATION` no barramento,
  mas esse evento só alimenta o HUD in-app (~~`apps/desktop/src/features/character/index.ts`~~ troca a expressão do avatar +
  ~~`apps/desktop/src/preload/mika-runtime-events.ts`~~ repassa pro renderer, que faz `addNotification` — um toast na
  tela, não uma notificação do SO). Não existe também nenhuma chave de config para um toggle de
  notificação do SO. O plugin `tauri-plugin-notification` continua registrado e sem consumidor.
  **Não portado** — a afirmação anterior ("quem notificava era o scheduler/proatividade") não se
  sustenta contra o fonte atual; fica corrigida aqui. Reviver = primeiro decidir, como produto, que
  a Mika deve notificar pelo SO (hoje ninguém pediu isso), então adicionar o toggle de config e o
  consumidor junto.
- **Leitura de área de transferência** — atualizado em 2026-09-22: o runtime existe agora
  (`modules/vision/clipboard_watch.rs::spawn_clipboard_watch`), lendo via `tauri-plugin-clipboard-manager`
  (`ClipboardExt::read_image`), gateado pelos TRÊS booleanos (`vision.enabled` +
  `vision.readClipboardImages` + `privacy.clipboardLogging`), com dedup via
  `image_hash::compute_image_hash` e publicação de `EventKey::VisionClipboardImage`. Ainda **não
  roda em produção**: falta a chamada em `modules/mod.rs::register_config_bound_tools`
  (`vision::spawn_clipboard_watch(Arc::clone(&store), crate::events::bus(), app.clone());`), fora do
  escopo de quem porta `modules/vision/**`. A análise por LLM da imagem colada
  (`BRAIN_PROACTIVE_THOUGHT` no TS) também fica de fora — a chave de evento não existe em
  `events/keys.rs` ainda.
- **Rebind de atalho pelo usuário** (`register`/`unregister`/`unregister_all` em `shortcuts.rs`).
  Reconfirmado em 2026-09-22: os três atalhos fixos (`F9`, `CommandOrControl+Space`,
  `CommandOrControl+Shift+M`) continuam registrados no boot em `register_app_shortcuts`, e o quarto
  atalho fixo do TS (mute de fala, `CommandOrControl+Shift+S` em ~~`apps/desktop/src/main/infrastructure/mute-shortcut.ts`~~)
  nem chegou a ser portado — nenhum dos dois é o que falta aqui. O host adapter
  `registerShortcut`/`unregisterShortcut` (~~`packages/sdk/src/host/adapters.ts`~~) só tem um chamador
  em produção — ~~`apps/desktop/src/main/windows/main-window-shortcuts.ts`~~, chamado uma vez no BOOT para registrar os
  quatro aceleradores fixos — e nenhuma feature, painel de settings ou schema de config (`grep` por
  `shortcut`/`accelerator` em `apps/desktop/src/features` e nos `*.schema.ts` não bate em nada) pede
  para trocar um acelerador em runtime. Rota sob demanda existe? Não — foi removida com o sidecar
  porque o único chamador dela era o próprio sidecar. **Não portado** — sem UI/settings key que
  precise disso, implementar rebind seria capacidade sem chamador; volta junto com quem primeiro
  pedir essa tela.
- **A cadeia de dedup da visão** — atualizado em 2026-09-22: não está mais dormente.
  `modules/vision/image_hash.rs`/`text_density.rs` agora têm chamador real,
  `modules/vision/watcher.rs::spawn_screen_watcher` (o observador periódico de tela, port funcional
  de ~~`apps/desktop/src/main/services/vision/capture/screen-watcher.service.ts`~~: gate de observação, dedup por hash, gate de densidade de
  texto, análise via `llm_client::analyze`, publicação de `EventKey::ScreenObservation`). Ainda
  **não roda em produção**: falta a chamada em `modules/mod.rs::register_config_bound_tools`
  (`vision::spawn_screen_watcher(Arc::clone(&store), crate::events::bus(), app.clone());`), mesmo
  motivo do item de clipboard acima — fora do escopo de quem porta `modules/vision/**`.
